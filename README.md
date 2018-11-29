# Mailgun Contact Form
## About
This is a very simple project to do the following:

* Receive a POST from an HTML form - expected parameters are
  * `from_name`
  * `from_email`
  * `title`
  * `body`
  
  And the `Content-Type` should be `application/x-www-form-urlencoded`.
* Convert that POST into an email to a pre-defined email address, via [Mailgun](https://www.mailgun.com)'s API
* Redirect to a predefined URL with the parameters `status` and `message` (if status is `error`)

That's it - it's *very* simple
## Building
### Own architecture
```bash
cargo build --release
```

### Cross-compiling for Linux & MUSL from macOS
```bash
brew install filosottile/musl-cross/musl-cross
CC_x86_64_unknown_linux_musl="x86_64-linux-musl-gcc" cargo build --release --target=x86_64-unknown-linux-musl
```

### Cross-compiling for MUSL from Ubuntu
```bash
sudo apt-get install musl-tools
CC_x86_64_unknown_linux_musl="musl-gcc" cargo build --release --target=x86_64-unknown-linux-musl
```

## Running
```bash
MAILGUN_API_KEY="<insert api key>" \
    MAILGUN_DOMAIN="<insert mailgun domain>" \
    MAILGUN_TO_ADDRESS="<insert address to send posted data to>" \
    MAILGUN_REDIRECT_URL="<HTTP address to redirect with 303 to after processing POST>" \
    ./target/release/mailgun-contact-form
```
## Other environment variables
* `RUST_LOG`: Control the logging level. Set to `actix_web=info,mailgun_contact_form=info` to get basic logging for the 
  web framework and the application
* `BIND_ADDRESS`: The address to bind to. Defaults to `0.0.0.0`
* `PORT`: The port to bind to. Defaults to `8088`