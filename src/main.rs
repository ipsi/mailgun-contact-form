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

use env_logger::{Builder, Target};
use std::error::Error;
use axum::{Form, Json, Router};
use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use log::{error, info};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use tower_http::cors::{Any, CorsLayer};

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

#[derive(Serialize)]
enum ResponseStatus {
    Ok,
    MailAgentError,
    InternalError,
}

#[derive(Serialize)]
struct ResponseData {
    status: ResponseStatus,
    message: Option<String>,
}

#[derive(Deserialize)]
struct MailGunErrorResponse {
    message: String,
}

lazy_static!(
    static ref API_KEY: String = std::env::var("MAILGUN_API_KEY").unwrap();
    static ref DOMAIN: String = std::env::var("MAILGUN_DOMAIN").unwrap();
    static ref TO: String = std::env::var("MAILGUN_TO_ADDRESS").unwrap();
    static ref HOST: String = format!("https://api.mailgun.net/v3/{}/messages", DOMAIN.as_str());
    static ref CLIENT: reqwest::Client = reqwest::Client::new();
);

enum ContactFormError {
    MailGunError(reqwest::Error),
}

impl From<reqwest::Error> for ContactFormError {
    fn from(e: reqwest::Error) -> Self {
        ContactFormError::MailGunError(e)
    }
}

impl IntoResponse for ContactFormError {
    fn into_response(self) -> Response {
        match self {
            ContactFormError::MailGunError(e) => {
                error!("Error sending mail: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(ResponseData { status: ResponseStatus::InternalError, message: Some(format!("{}", e)) })).into_response()
            }
        }
    }
}

async fn send_form(Form(req): Form<FormData>) -> Result<impl IntoResponse, ContactFormError> {
    let base_from = format!("{} <{}>", req.from_name, req.from_email);
    info!("Sending mail from [{}]", base_from.as_str());
    let from = base_from.as_str();
    let data = MailGunData {
        from,
        to: &TO,
        subject: &req.title,
        text: &req.body,
    };
    let response = CLIENT.post(HOST.as_str())
        .basic_auth("api", Some(API_KEY.as_str()))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&data)
        .send()
        .await?;

    match response {
        response if response.status().is_success() => {
            info!("Mail sent successfully");
            Ok((StatusCode::OK, Json(ResponseData { status: ResponseStatus::Ok, message: None })))
        }
        response if response.status() == StatusCode::UNAUTHORIZED => {
            let body = response.text().await?;
            info!("Received a 401 error trying to call MailGun: {}", body);
            Ok((StatusCode::INTERNAL_SERVER_ERROR, Json(ResponseData { status: ResponseStatus::MailAgentError, message: Some("error communicating with mail agent".to_string()) })))
        }
        response => {
            let data = response.json::<MailGunErrorResponse>().await?;
            error!("Mailgun error: {}", data.message);
            Ok((StatusCode::BAD_GATEWAY, Json(ResponseData { status: ResponseStatus::MailAgentError, message: Some("error communicating with mail agent".to_string()) })))
        }
    }
}

const DEFAULT_PORT: &'static str = "8088";
const DEFAULT_BIND_ADDRESS: &'static str = "0.0.0.0";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut builder = Builder::from_default_env();
    builder.target(Target::Stdout);

    builder.init();
    // Check env vars now so we don't get a panic later!
    std::env::var("MAILGUN_API_KEY").map_err(|_| "Environment variable \"MAILGUN_API_KEY\" must be present")?;
    std::env::var("MAILGUN_DOMAIN").map_err(|_| "Environment variable \"MAILGUN_DOMAIN\" must be present")?;
    std::env::var("MAILGUN_TO_ADDRESS").map_err(|_| "Environment variable \"MAILGUN_TO_ADDRESS\" must be present")?;

    // Load lazy statics right away - they're only lazy because they can't be evaluated at compile time!
    info!("Will be sending mail via domain {}, to address {}, with API key starting with {}", *DOMAIN, *TO, &API_KEY[0..6]);

    let bind_address = std::env::var("BIND_ADDRESS").unwrap_or(DEFAULT_BIND_ADDRESS.to_string());
    let port = std::env::var("PORT").unwrap_or(DEFAULT_PORT.to_string());

    info!("Binding to {}:{}", bind_address, port);

    let cors = CorsLayer::new()
        // allow `GET` and `POST` when accessing the resource
        .allow_methods([Method::GET, Method::POST])
        // allow requests from any origin
        .allow_origin(Any);

    let app = Router::new().route("/", post(send_form)).layer(cors);

    axum::Server::bind(&format!("{}:{}", bind_address, port).parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();

    Ok(())
}
