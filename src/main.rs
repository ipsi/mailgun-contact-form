/*
 * BSD 3-Clause License
 *
 * Copyright (c) 2018, Andrew Thorburn All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without modification, are permitted
 * provided that the following conditions are met:
 *
 * Redistributions of source code must retain the above copyright notice, this list of conditions
 * and the following disclaimer.
 *
 * Redistributions in binary form must reproduce the above copyright notice, this list of conditions
 * and the following disclaimer in the documentation and/or other materials provided with the
 * distribution.
 *
 * Neither the name of the copyright holder nor the names of its contributors may be used to endorse
 * or promote products derived from this software without specific prior written permission.
 *
 * THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR
 * IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND
 * FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR
 * CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
 * DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
 * DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY,
 * WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY
 * WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 */

extern crate actix_web;
extern crate futures;
extern crate serde_urlencoded;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate log;
extern crate env_logger;
extern crate url;

use actix_web::{
    server,
    client,
    App,
    Form,
    http,
    HttpMessage,
    HttpResponse,
    Responder,
};
use actix_web::middleware::{
    Logger,
};
use actix_web::http::{
    header::{HeaderValue},
    StatusCode,
};
use actix_web::client::SendRequestError;
use env_logger::{Builder, Target};
use futures::Future;
use futures::future::{
    Either,
    FutureResult,
};
use url::percent_encoding::{utf8_percent_encode, DEFAULT_ENCODE_SET};
use actix_web::error::{
    JsonPayloadError,
    PayloadError,
};
use std::error::Error;

#[derive(Deserialize)]
struct FormData {
    from_name: String,
    from_email: String,
    title: String,
    body: String,
}

#[derive(Serialize)]
struct MailGunData<'a> {
    from: &'a str,
    to: &'a str,
    subject: &'a str,
    text: &'a str,
}

#[derive(Deserialize)]
struct MailGunErrorResponse {
    message: String,
}

#[derive(Debug)]
struct ResponseError(String);

impl Error for ResponseError {
    fn description(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ResponseError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        f.write_str(&self.0)
    }
}

impl actix_web::ResponseError for ResponseError {
    fn error_response(&self) -> actix_web::HttpResponse {
        let mut response = actix_web::HttpResponse::new(http::StatusCode::INTERNAL_SERVER_ERROR);
        response.set_body(self.0.clone());
        response
    }
}

impl From<PayloadError> for ResponseError {
    fn from(err: PayloadError) -> Self {
        ResponseError(format!("{}", err))
    }
}

impl From<SendRequestError> for ResponseError {
    fn from(err: SendRequestError) -> Self {
        ResponseError(format!("{}", err))
    }
}

impl From<JsonPayloadError> for ResponseError {
    fn from(err: JsonPayloadError) -> Self {
        ResponseError(format!("{}", err))
    }
}

lazy_static!(
    static ref API_KEY: String = std::env::var("MAILGUN_API_KEY").unwrap();
    static ref DOMAIN: String = std::env::var("MAILGUN_DOMAIN").unwrap();
    static ref TO: String = std::env::var("MAILGUN_TO_ADDRESS").unwrap();
    static ref REDIRECT_URL: String = std::env::var("MAILGUN_REDIRECT_URL").unwrap();
    static ref BASIC_AUTH: String = base64::encode(&format!("{}:{}", "api", API_KEY.as_str()));
    static ref HOST: String = format!("https://api.mailgun.net/v3/{}/messages", DOMAIN.as_str());
    static ref AUTH_HEADER: String = format!("Basic {}", BASIC_AUTH.as_str());
);

fn create_ok_response<E>() -> FutureResult<HttpResponse, E> {
    let mut response_builder = HttpResponse::build(StatusCode::SEE_OTHER).finish();
    {
        let headers_mut = response_builder.headers_mut();
        headers_mut.append(
            "Location",
            HeaderValue::from_str(
                format!(
                    "{}?status={}",
                    REDIRECT_URL.as_str(),
                    "success"
                ).as_str()
            ).unwrap()
        );
    }
    futures::finished(response_builder)
}

fn create_err_response<E>(body: &str) -> FutureResult<HttpResponse, E> {
    let mut response_builder = HttpResponse::build(StatusCode::SEE_OTHER).finish();
    {
        let headers_mut = response_builder.headers_mut();
        headers_mut.append(
            "Location",
            HeaderValue::from_str(
                format!(
                    "{}?status={}&message={}",
                    REDIRECT_URL.as_str(),
                    "error",
                    utf8_percent_encode(body, DEFAULT_ENCODE_SET).to_string()
                ).as_str()
            ).unwrap()
        );
    }
    futures::finished(response_builder)
}

fn send_form(req: Form<FormData>) -> Result<Box<Future<Item = impl Responder, Error = ResponseError>>, actix_web::Error> {
    let base_from = format!("{} <{}>", req.from_name, req.from_email);
    info!("Sending mail from [{}]", base_from.as_str());
    let from = base_from.as_str();
    let data = MailGunData {
        from,
        to: &TO,
        subject: &req.title,
        text: &req.body,
    };
    Ok(Box::new(client::post(HOST.as_str())
        .header("Authorization", AUTH_HEADER.as_str())
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&data)?
        .send()
        .from_err::<ResponseError>()
        .and_then(|resp| {
            info!("Received response with status {}", resp.status());
            return if resp.status().is_success() {
                Either::A(create_ok_response())
            } else if resp.status() == StatusCode::UNAUTHORIZED { // Doesn't return a JSON response on a 401...
                error!("Received a 401 error trying to call MailGun...");
                let f = resp.body().and_then(|raw_body| {
                    let body = String::from_utf8_lossy(raw_body.as_ref()).to_string();
                    create_err_response(&body)
                }).from_err();
                Either::B(Either::A(f))
            } else {
                let f = resp.json().from_err::<ResponseError>().and_then(|body: MailGunErrorResponse| {
                    error!("Received an error from MailGun: {}", body.message);
                    create_err_response(&body.message)
                }).from_err();
                Either::B(Either::B(f))
            }
        })
        .or_else(|err: ResponseError| {
            error!("Received an error processing the request: {}", err);
            create_err_response(err.description())
        })
    ))
}

const DEFAULT_PORT: &'static str = "8088";
const DEFAULT_BIND_ADDRESS: &'static str = "0.0.0.0";

fn main() -> Result<(), Box<Error>> {
    let mut builder = Builder::from_default_env();
    builder.target(Target::Stdout);

    builder.init();
    // Check env vars now so we don't get a panic later!
    std::env::var("MAILGUN_API_KEY").map_err(|_| "Environment variable \"MAILGUN_API_KEY\" must be present")?;
    std::env::var("MAILGUN_DOMAIN").map_err(|_| "Environment variable \"MAILGUN_DOMAIN\" must be present")?;
    std::env::var("MAILGUN_TO_ADDRESS").map_err(|_| "Environment variable \"MAILGUN_TO_ADDRESS\" must be present")?;
    std::env::var("MAILGUN_REDIRECT_URL").map_err(|_| "Environment variable \"MAILGUN_REDIRECT_URL\" must be present")?;

    // Load lazy statics right away - they're only lazy because they can't be evaluated at compile time!
    info!("Will be sending mail via domain {}, to address {}, with API key starting with {}, and redirecting to {} after sending mail", *DOMAIN, *TO, &API_KEY[0..6], *REDIRECT_URL);

    let bind_address = std::env::var("BIND_ADDRESS").unwrap_or(DEFAULT_BIND_ADDRESS.to_string());
    let port = std::env::var("PORT").unwrap_or(DEFAULT_PORT.to_string());

    info!("Binding to {}:{}", bind_address, port);

    server::new(|| App::new().middleware(Logger::default()).route("/", http::Method::POST, send_form))
        .bind(format!("{}:{}", &bind_address, &port))?
        .run();

    Ok(())
}
