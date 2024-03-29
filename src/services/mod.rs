pub mod provider;
pub mod publisher;

use crate::config::Config;
use crate::helpers::{api, database, email, handler};

use actix::System;
use actix_web::middleware::errhandlers::ErrorHandlers;
use actix_web::{http, web, App, HttpServer};
use carapax::prelude::{UpdateMethod, UpdatesStream};
use std::sync::Arc;

pub fn init_services(cnfg: Arc<Config>) {
    let sys = System::new("aunema");
    let db_pool = database::init_pool(&cnfg, 5).expect("Failed to init database connection");

    let addr = format!("0.0.0.0:{}", cnfg.server_port);
    let token = String::from("token");

    let mailer = email::init_mailer(&cnfg).expect("Failed to init mailer");
    // Todo: Send db_pool to telegram setupd
    let mut telegram = api::init_telegram(token).expect("Failed to init telegram api");

    let provider_ucs = provider::usecase::init(&cnfg, &db_pool);
    let publisher_ucs = publisher::usecase::init(&cnfg, &db_pool);

    let provider_cnr = provider::controller::init(&cnfg, &provider_ucs, &mailer);
    let publisher_cnr = publisher::controller::init(&cnfg, &publisher_ucs);

    telegram = provider::delivery::telegram::init(&cnfg, &provider_cnr, telegram);
    telegram = publisher::delivery::telegram::init(&cnfg, &publisher_cnr, telegram);
    let app = move || {
        let provider_dlr_rest = provider::delivery::rest::init(&cnfg, &provider_cnr);

        let api = web::scope("/api/v1").service(provider_dlr_rest);

        App::new()
            .wrap(
                ErrorHandlers::new()
                    .handler(http::StatusCode::BAD_REQUEST, handler::bad_request_handler),
            )
            .service(api)
    };

    telegram.app = telegram.app.add_handler(telegram.handler);
    actix::spawn(telegram.app.run(
        telegram.api.clone(),
        UpdateMethod::poll(UpdatesStream::new(telegram.api.clone())),
    ));
    HttpServer::new(app)
        .bind(addr)
        .expect("Failed to bind port for the http server")
        .start();
    sys.run().expect("Failed to run actix system");
}
