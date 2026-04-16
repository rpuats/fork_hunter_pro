pub mod parser_health;

#[cfg(feature = "full")]
pub mod handlers;
#[cfg(feature = "full")]
pub mod routes;
#[cfg(feature = "full")]
pub mod ws;

#[cfg(feature = "full")]
pub use routes::create_router;
