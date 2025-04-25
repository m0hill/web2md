use std::borrow::Cow;

pub struct MetadataHandler {
    pub title: Option<Cow<'static, str>>,
    pub author: Option<Cow<'static, str>>,
    pub date: Option<Cow<'static, str>>,
    pub description: Option<Cow<'static, str>>,
    pub tags: Vec<Cow<'static, str>>,
    metadata_buffer: String,
}

impl MetadataHandler {
    pub fn new() -> Self {
        Self {
            title: None,
            author: None,
            date: None,
            description: None,
            tags: Vec::with_capacity(10),
            metadata_buffer: String::with_capacity(1024),
        }
    }

    pub fn format_metadata(&mut self) -> &str {
        self.metadata_buffer.clear();

        if let Some(title) = &self.title {
            self.metadata_buffer.push_str("# ");
            self.metadata_buffer.push_str(title);
            self.metadata_buffer.push_str("\n\n");
        }

        self.metadata_buffer.push_str("---\n");

        if let Some(author) = &self.author {
            self.metadata_buffer.push_str("Author: ");
            self.metadata_buffer.push_str(author);
            self.metadata_buffer.push('\n');
        }
        if let Some(date) = &self.date {
            self.metadata_buffer.push_str("Date: ");
            self.metadata_buffer.push_str(date);
            self.metadata_buffer.push('\n');
        }
        if let Some(description) = &self.description {
            self.metadata_buffer.push_str("Description: ");
            self.metadata_buffer.push_str(description);
            self.metadata_buffer.push('\n');
        }
        if !self.tags.is_empty() {
            self.metadata_buffer.push_str("Tags: ");
            for (i, tag) in self.tags.iter().enumerate() {
                if i > 0 {
                    self.metadata_buffer.push_str(", ");
                }
                self.metadata_buffer.push_str(tag);
            }
            self.metadata_buffer.push('\n');
        }

        self.metadata_buffer.push_str("---\n\n");
        &self.metadata_buffer
    }
}