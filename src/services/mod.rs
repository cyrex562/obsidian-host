pub mod file_service;
pub mod frontmatter_service;
pub mod markdown_service;
pub mod search_service;
pub mod wiki_link_service;

pub use file_service::{FileService, RenameStrategy};
pub use markdown_service::{MarkdownService, RenderOptions};
pub use search_service::SearchIndex;
pub use wiki_link_service::{FileIndex, ResolvedLink, WikiLinkResolver};
