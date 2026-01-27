use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "frontend/public/"]
pub struct Assets;
