use html5ever::parse_document;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::borrow::Cow;
use std::cell::RefCell;
use regex::Regex;
use crate::config::{ConvertConfig, HtmlConversionResult};
use crate::metadata::MetadataHandler;

#[derive(Debug, Clone, Copy, PartialEq)]
enum BlockType {
    Paragraph,
    Header(u8),
    List(ListType),
    CodeBlock,
    Table,
    Quote,
    Pre,
    Div,
    Article,
    Section,
    TableRow,
    TableCell,
    TableHeader,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ListType {
    Ordered(u8),
    Unordered,
}

lazy_static! {
    static ref INLINE_TAGS: HashMap<&'static str, (&'static str, &'static str)> = {
        let mut m = HashMap::new();
        m.insert("strong", ("**", "**"));
        m.insert("b", ("**", "**"));
        m.insert("em", ("*", "*"));
        m.insert("i", ("*", "*"));
        m.insert("code", ("`", "`"));
        m.insert("mark", ("==", "=="));
        m.insert("del", ("~~", "~~"));
        m.insert("ins", ("__", "__"));
        m
    };

    static ref BLOCK_TAGS: HashMap<&'static str, BlockType> = {
        let mut m = HashMap::new();
        m.insert("p", BlockType::Paragraph);
        m.insert("div", BlockType::Div);
        m.insert("article", BlockType::Article);
        m.insert("section", BlockType::Section);
        m.insert("table", BlockType::Table);
        m.insert("tr", BlockType::TableRow);
        m.insert("td", BlockType::TableCell);
        m.insert("th", BlockType::TableHeader);
        m
    };
     static ref WHITESPACE_REGEX: Regex = Regex::new(r"\s+").unwrap();
}

struct MarkdownFormatter<'a> {
    config: ConvertConfig,
    content: String,
    indent_level: usize,
    list_type_stack: Vec<ListType>,
    block_stack: Vec<BlockType>,
    last_block_type: Option<BlockType>,
    in_table: bool,
    table_columns: Vec<Cow<'a, str>>,
    table_rows: Vec<Vec<Cow<'a, str>>>,
    current_row: Vec<Cow<'a, str>>,
    current_cell: String,
    metadata: MetadataHandler,
    in_code_block: bool,
    text_buffer: String,
    link_buffer: String,
    table_buffer: String,
    last_was_block: bool,
    preserve_next_whitespace: bool,
    line_prefix: String,
    temp_buffer: String,
    format_buffer: String,
    node_buffer: String,
    links: Vec<String>,
}

impl<'a> MarkdownFormatter<'a> {
    fn new(config: ConvertConfig) -> Self {
        Self {
            config,
            content: String::with_capacity(16384),
            indent_level: 0,
            list_type_stack: Vec::with_capacity(8),
            block_stack: Vec::with_capacity(16),
            last_block_type: None,
            in_table: false,
            table_columns: Vec::with_capacity(8),
            table_rows: Vec::with_capacity(20),
            current_row: Vec::with_capacity(8),
            current_cell: String::with_capacity(256),
            metadata: MetadataHandler::new(),
            in_code_block: false,
            text_buffer: String::with_capacity(2048),
            link_buffer: String::with_capacity(256),
            table_buffer: String::with_capacity(4096),
            last_was_block: false,
            preserve_next_whitespace: false,
            line_prefix: String::with_capacity(32),
            temp_buffer: String::with_capacity(1024),
            format_buffer: String::with_capacity(1024),
            node_buffer: String::with_capacity(2048),
            links: Vec::new(),
        }
    }

    fn add_block_spacing(&mut self, block_type: BlockType) {
        match block_type {
            BlockType::Header(_) => {
                if !self.content.ends_with("\n\n") {
                    self.add_double_newline();
                }
            }
            BlockType::Paragraph => {
                if !self.last_was_block {
                    self.add_double_newline();
                }
            }
            BlockType::List(list_type) => {
                if self.last_block_type != Some(BlockType::List(list_type)) {
                    self.add_newline();
                }
            }
            BlockType::CodeBlock | BlockType::Pre => {
                self.add_double_newline();
                self.preserve_next_whitespace = true;
            }
            BlockType::Quote => {
                if !self.content.ends_with('\n') {
                    self.add_newline();
                }
            }
            _ => if !self.content.ends_with('\n') {
                self.add_newline();
            }
        }

        self.last_block_type = Some(block_type);
        self.last_was_block = true;
    }

    fn should_skip_node(&self, handle: &Handle) -> bool {
        if !self.config.cleaning_rules.remove_scripts
           && !self.config.cleaning_rules.remove_styles
           && !self.config.cleaning_rules.remove_comments {
            return false;
        }

        match &handle.data {
            NodeData::Element { name, .. } => {
                let tag = name.local.as_ref();
                (self.config.cleaning_rules.remove_scripts && tag == "script") ||
                (self.config.cleaning_rules.remove_styles && tag == "style")
            }
            NodeData::Comment { .. } => self.config.cleaning_rules.remove_comments,
            NodeData::ProcessingInstruction { .. } => true,
            _ => false
        }
    }

    fn process_table_cell(&mut self, handle: &Handle) {
        self.current_cell.clear();
        self.current_cell.reserve(64);

        self.process_children(handle);

        let cell_content = self.current_cell.trim();
        if cell_content.is_empty() {
            self.current_row.push(Cow::Borrowed(""));
        } else {
            let cleaned = if self.config.clean_whitespace {
                let needs_cleaning = cell_content.contains(|c: char| c.is_whitespace()) &&
                                   !cell_content.chars().all(char::is_whitespace);

                if needs_cleaning {
                    self.node_buffer.clear();
                    self.node_buffer.reserve(cell_content.len());

                    let mut last_was_space = false;
                    for c in cell_content.chars() {
                        if c.is_whitespace() {
                            if !last_was_space {
                                self.node_buffer.push(' ');
                                last_was_space = true;
                            }
                        } else {
                            self.node_buffer.push(c);
                            last_was_space = false;
                        }
                    }
                    Cow::Owned(self.node_buffer.clone())
                } else {
                    Cow::Owned(cell_content.to_string())
                }
            } else {
                Cow::Owned(cell_content.to_string())
            };

            self.current_row.push(cleaned);
        }
    }

    fn clean_text<'b>(&mut self, text: &'b str) -> Cow<'b, str> {
        if !self.config.clean_whitespace || self.in_code_block || self.preserve_next_whitespace {
            self.preserve_next_whitespace = false;
            return Cow::Borrowed(text);
        }

        let trimmed = text.trim();
        if trimmed.is_empty() || trimmed.chars().all(char::is_whitespace) {
            return Cow::Borrowed("");
        }

        let needs_cleaning = trimmed.contains(|c: char| c.is_whitespace()) &&
                           !trimmed.chars().all(char::is_whitespace);

        if !needs_cleaning {
            return Cow::Borrowed(trimmed);
        }

        self.temp_buffer.clear();
        self.temp_buffer.reserve(trimmed.len());

        let mut last_was_space = false;
        let mut last_was_newline = false;

        let mut chars = trimmed.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '\n' => {
                    if self.config.cleaning_rules.preserve_line_breaks && !last_was_newline {
                        self.temp_buffer.push('\n');
                        last_was_newline = true;
                        last_was_space = false;
                    } else if !last_was_space {
                        self.temp_buffer.push(' ');
                        last_was_space = true;
                    }
                }
                c if c.is_whitespace() => {
                    if !last_was_space {
                        self.temp_buffer.push(' ');
                        last_was_space = true;
                    }
                    last_was_newline = false;
                }
                c => {
                    self.temp_buffer.push(c);
                    last_was_space = false;
                    last_was_newline = false;
                }
            }
        }

        Cow::Owned(self.temp_buffer.clone())
    }

    fn process_node(&mut self, handle: &Handle) {
        if self.should_skip_node(handle) {
            return;
        }

        match &handle.data {
            NodeData::Element { name, attrs, .. } => {
                let tag_name = name.local.as_ref();

                match tag_name {
                    name @ ("h1" | "h2" | "h3" | "h4" | "h5" | "h6") => {
                        if self.config.preserve_headings {
                            let level = name[1..].parse::<u8>().unwrap();
                            if level <= self.config.max_heading_level {
                                self.block_stack.push(BlockType::Header(level));
                                self.add_block_spacing(BlockType::Header(level));
                                self.process_header(handle, level);
                                self.block_stack.pop();
                            }
                        }
                    }

                    "p" => {
                        self.block_stack.push(BlockType::Paragraph);
                        self.add_block_spacing(BlockType::Paragraph);
                        self.process_children(handle);
                        self.block_stack.pop();
                        self.add_newline();
                    }

                    "pre" => {
                        self.block_stack.push(BlockType::Pre);
                        self.add_block_spacing(BlockType::Pre);
                        self.process_code_block(handle, attrs);
                        self.block_stack.pop();
                    }

                    "blockquote" => {
                        self.block_stack.push(BlockType::Quote);
                        self.add_block_spacing(BlockType::Quote);
                        self.process_quote(handle);
                        self.block_stack.pop();
                    }

                    "a" => self.process_link(handle, attrs),
                    "img" => self.process_image(handle, attrs),
                    "meta" if self.config.include_metadata => self.extract_metadata(handle, attrs),

                    "code" => self.process_inline_code(handle),
                    "table" => self.process_table(handle),
                    "tr" if self.in_table => {
                        self.current_row.clear();
                        self.process_children(handle);
                        if !self.current_row.is_empty() {
                            let mut new_row = Vec::with_capacity(self.current_row.len());
                            new_row.extend_from_slice(&self.current_row);
                            self.table_rows.push(new_row);
                        }
                    },
                    "th" | "td" if self.in_table => self.process_table_cell(handle),

                    "ul" => self.process_list(handle, ListType::Unordered),
                    "ol" => self.process_list(handle, ListType::Ordered(1)),

                    tag if INLINE_TAGS.contains_key(tag) => {
                        let (prefix, suffix) = INLINE_TAGS[tag];
                        self.content.push_str(prefix);
                        self.process_children(handle);
                        self.content.push_str(suffix);
                    }

                    tag if BLOCK_TAGS.contains_key(tag) => {
                        self.add_double_newline();
                        self.process_children(handle);
                        self.add_double_newline();
                    }

                    _ => self.process_children(handle),
                }
            }

            NodeData::Text { contents } => {
                let text = contents.borrow();
                let text_content = if self.config.clean_whitespace && !self.in_code_block {
                    self.clean_text(&text).into_owned()
                } else {
                    text.to_string()
                };

                if self.in_table {
                    self.current_cell.push_str(&text_content);
                } else {
                    self.content.push_str(&text_content);
                }
            }

            _ => self.process_children(handle),
        }
    }

    fn process_quote(&mut self, handle: &Handle) {
        let old_prefix = self.line_prefix.clone();
        self.line_prefix.push_str("> ");

        self.content.push_str(&self.line_prefix);
        self.process_children(handle);

        if !self.content.ends_with('\n') {
            self.add_newline();
        }

        self.line_prefix = old_prefix;
    }

    fn process_code_block(&mut self, handle: &Handle, attrs: &RefCell<Vec<html5ever::Attribute>>) {
        self.block_stack.push(BlockType::CodeBlock);
        self.in_code_block = true;
        self.add_double_newline();
        self.content.push_str("```");

        if let Some(class) = attrs.borrow().iter()
            .find(|attr| attr.name.local.as_ref() == "class")
            .map(|attr| attr.value.as_ref())
        {
            if let Some(lang) = class.split_whitespace()
                .find(|c| c.starts_with("language-"))
            {
                self.content.push_str(&lang[9..]);
            }
        }

            self.content.push('\n');
            self.process_children(handle);
            self.content.push_str("\n```");
            self.add_newline();
            self.in_code_block = false;
            self.block_stack.pop();
        }

    fn process_inline_code(&mut self, handle: &Handle) {
        let was_in_code = self.in_code_block;
        self.in_code_block = true;
        self.content.push('`');
        self.process_children(handle);
        self.content.push('`');
        self.in_code_block = was_in_code;
    }

    fn process_header(&mut self, handle: &Handle, level: u8) {
        self.add_double_newline();
        self.content.push_str(&"#".repeat(level as usize));
        self.content.push(' ');
        self.process_children(handle);
        self.add_double_newline();
    }

    fn process_link(&mut self, handle: &Handle, attrs: &RefCell<Vec<html5ever::Attribute>>) {
        if !self.config.include_links {
            self.process_children(handle);
            return;
        }

        if let Some(ref href) = attrs.borrow().iter()
            .find(|attr| attr.name.local.as_ref() == "href")
            .map(|attr| attr.value.to_string())
        {
            self.link_buffer.clear();
            let content_len = self.content.len();
            self.process_children(handle);
            self.link_buffer.clear();
            self.link_buffer.push_str(&self.content[content_len..]);
            self.content.truncate(content_len);

            if !self.link_buffer.is_empty() && self.link_buffer != *href {
                self.content.push('[');
                self.content.push_str(&self.link_buffer);
                self.content.push_str("](");
                self.content.push_str(href);
                self.content.push(')');
            } else {
                self.content.push('<');
                self.content.push_str(href);
                self.content.push('>');
            }

            self.links.push(href.to_string());
        }
    }

    fn process_table(&mut self, handle: &Handle) {
        self.in_table = true;
        self.table_columns.clear();
        self.table_rows.clear();
        self.table_buffer.clear();

        self.process_children(handle);

        if !self.table_rows.is_empty() {
            let owned_rows: Vec<Vec<String>> = self.table_rows.iter()
                .map(|row| row.iter().map(|cow| cow.to_string()).collect())
                .collect();

            let col_count = owned_rows.iter().map(|r| r.len()).max().unwrap_or(0);
            let mut col_widths = vec![0; col_count];

            for row in &owned_rows {
                for (i, cell) in row.iter().enumerate() {
                     if i < col_count {
                         col_widths[i] = col_widths[i].max(cell.len());
                     }
                }
            }

            self.add_double_newline();

            if let Some(header_row) = owned_rows.first() {
                self.format_buffer.clear();
                self.format_buffer.push('|');
                for (i, cell) in header_row.iter().enumerate() {
                    if i < col_widths.len() {
                        let padding = col_widths[i].saturating_sub(cell.len());
                        self.format_buffer.push(' ');
                        self.format_buffer.push_str(cell);
                        self.format_buffer.extend(std::iter::repeat(' ').take(padding));
                        self.format_buffer.push_str(" |");
                    }
                }
                // Pad remaining columns if header row is shorter
                for i in header_row.len()..col_count {
                     let padding = col_widths[i];
                     self.format_buffer.push(' ');
                     self.format_buffer.extend(std::iter::repeat(' ').take(padding));
                     self.format_buffer.push_str(" |");
                }
                self.format_buffer.push('\n');
                self.content.push_str(&self.format_buffer);

                self.format_buffer.clear();
                self.format_buffer.push('|');
                for width in &col_widths {
                    self.format_buffer.push_str(" ");
                    self.format_buffer.push_str(&"-".repeat(*width));
                    self.format_buffer.push_str(" |");
                }
                self.format_buffer.push('\n');
                self.content.push_str(&self.format_buffer);
            }

            for row in owned_rows.iter().skip(1) {
                self.format_buffer.clear();
                self.format_buffer.push('|');
                for (i, cell) in row.iter().enumerate() {
                    if i < col_widths.len() {
                        let padding = col_widths[i].saturating_sub(cell.len());
                        self.format_buffer.push(' ');
                        self.format_buffer.push_str(cell);
                        self.format_buffer.extend(std::iter::repeat(' ').take(padding));
                        self.format_buffer.push_str(" |");
                    }
                }
                 // Pad remaining columns if row is shorter
                for i in row.len()..col_count {
                     let padding = col_widths[i];
                     self.format_buffer.push(' ');
                     self.format_buffer.extend(std::iter::repeat(' ').take(padding));
                     self.format_buffer.push_str(" |");
                }
                self.format_buffer.push('\n');
                self.content.push_str(&self.format_buffer);
            }

            self.add_newline();
        }

        self.in_table = false;
    }


    fn process_image(&mut self, _handle: &Handle, attrs: &RefCell<Vec<html5ever::Attribute>>) {
        let attrs = attrs.borrow();
        let src = attrs.iter()
            .find(|attr| attr.name.local.as_ref() == "src")
            .map(|attr| attr.value.as_ref());

        let alt = attrs.iter()
            .find(|attr| attr.name.local.as_ref() == "alt")
            .map(|attr| attr.value.as_ref())
            .unwrap_or_default();

        if let Some(url) = src {
            self.add_newline();
            self.content.push_str("![");
            self.content.push_str(alt);
            self.content.push_str("](");
            self.content.push_str(url);
            self.content.push(')');
            self.add_newline();
        }
    }

    fn process_list(&mut self, handle: &Handle, list_type: ListType) {
        self.block_stack.push(BlockType::List(list_type));
        self.list_type_stack.push(list_type);
        self.indent_level += match list_type {
            ListType::Unordered => 2,
            ListType::Ordered(_) => 3,
        };

        self.text_buffer.clear();
        self.text_buffer.reserve(self.indent_level + 4);

        let mut current_count = match list_type {
            ListType::Ordered(start) => start,
            _ => 1, // Default start for unordered or if start is not specified
        };

        for child in handle.children.borrow().iter() {
            if let NodeData::Element { ref name, .. } = child.data {
                if name.local.as_ref() == "li" {
                    self.text_buffer.clear();
                    // Calculate indent based on current stack depth for nested lists
                    let current_indent = self.list_type_stack.iter().fold(0, |acc, lt| {
                        acc + match lt {
                            ListType::Unordered => 2,
                            ListType::Ordered(_) => 3,
                        }
                    }) - match list_type { // Subtract current level's base indent before adding prefix
                        ListType::Unordered => 2,
                        ListType::Ordered(_) => 3,
                    };

                    self.text_buffer.push_str(&" ".repeat(current_indent));


                    match list_type {
                        ListType::Unordered => {
                            self.text_buffer.push_str("* ");
                        },
                        ListType::Ordered(_) => {
                            // Corrected variable name from ¤t_count to current_count
                            self.text_buffer.push_str(&current_count.to_string());
                            self.text_buffer.push_str(". ");
                        },
                    };

                    self.content.push_str(&self.text_buffer);
                     // Add a newline before processing child if content doesn't end with newline
                    // This helps separate list item content properly
                    if !self.content.ends_with('\n') && !self.content.is_empty() {
                        self.add_newline();
                    }
                    self.process_node(child);
                    // Ensure a newline after processing the list item's content
                    self.add_newline();

                    current_count += 1;
                } else {
                    // Handle non-<li> elements inside <ul>/<ol> if necessary,
                    // otherwise they might get processed without proper list context.
                    // For now, just process them as children.
                    self.process_node(child);
                }
            } else {
                 // Handle text nodes or comments directly inside <ul>/<ol>
                 self.process_node(child);
            }
        }

        self.block_stack.pop();
        self.list_type_stack.pop();
        // Indent level is managed by stack, no need to subtract manually here
        // self.indent_level -= match list_type { ... };
        self.add_newline(); // Add a newline after the list finishes
    }

    fn extract_metadata(&mut self, _handle: &Handle, attrs: &RefCell<Vec<html5ever::Attribute>>) {
        let mut property_value = None;
        let mut name_value = None;
        let mut content_value = None;

        // Need to handle cases like <meta name="description" content="...">
        // and <meta property="og:title" content="...">
        for attr in attrs.borrow().iter() {
            match attr.name.local.as_ref() {
                "property" => property_value = Some(attr.value.to_string()),
                "name" => name_value = Some(attr.value.to_string()),
                "content" => content_value = Some(attr.value.to_string()),
                _ => {}
            }
        }

        if let Some(content) = content_value {
             if let Some(property) = property_value {
                match property.as_str() {
                    "og:title" if self.metadata.title.is_none() => self.metadata.title = Some(Cow::Owned(content)),
                    "og:description" if self.metadata.description.is_none() => self.metadata.description = Some(Cow::Owned(content)),
                    "article:author" if self.metadata.author.is_none() => self.metadata.author = Some(Cow::Owned(content)),
                    "article:published_time" if self.metadata.date.is_none() => self.metadata.date = Some(Cow::Owned(content)),
                    "article:tag" => self.metadata.tags.push(Cow::Owned(content)),
                    _ => {}
                }
            } else if let Some(name) = name_value {
                 match name.as_str() {
                     "description" if self.metadata.description.is_none() => self.metadata.description = Some(Cow::Owned(content)),
                     "author" if self.metadata.author.is_none() => self.metadata.author = Some(Cow::Owned(content)),
                     "keywords" => {
                         // Split keywords and add as tags if not already present
                         for keyword in content.split(',') {
                             let trimmed_keyword = keyword.trim();
                             if !trimmed_keyword.is_empty() && !self.metadata.tags.iter().any(|t| t == trimmed_keyword) {
                                 self.metadata.tags.push(Cow::Owned(trimmed_keyword.to_string()));
                             }
                         }
                     }
                    _ => {}
                 }
            }
        }
    }

    fn process_children(&mut self, handle: &Handle) {
        for child in handle.children.borrow().iter() {
            self.process_node(child);
        }
    }

    fn add_newline(&mut self) {
        if !self.content.is_empty() && !self.content.ends_with('\n') {
            self.content.push('\n');
        }
    }

    fn add_double_newline(&mut self) {
        // Ensure there are exactly two newlines, trimming excess first
        while self.content.ends_with('\n') {
            self.content.pop();
        }
        self.content.push_str("\n\n");
    }


    fn result(mut self) -> HtmlConversionResult {
        let mut final_content = String::with_capacity(
            self.content.len() +
            if self.config.include_metadata { 1000 } else { 0 } // Estimate metadata size
        );

        if self.config.include_metadata {
             // Add title from <title> tag if OG title wasn't found
             // This needs access to the DOM root, which isn't easily available here.
             // Consider extracting title earlier or passing the DOM.
             // For now, relies only on meta tags.
            final_content.push_str(self.metadata.format_metadata());
        }

        // Trim starting/ending whitespace from the main content before adding
        let trimmed_content = self.content.trim();
        final_content.push_str(trimmed_content);


        let markdown = if self.config.clean_whitespace && !self.config.cleaning_rules.preserve_line_breaks {
            // Consolidate multiple blank lines into single blank lines (max two newlines)
            let mut cleaned = String::with_capacity(final_content.len());
            let mut newline_count = 0;
            for c in final_content.chars() {
                 if c == '\n' {
                     newline_count += 1;
                 } else {
                     newline_count = 0;
                 }

                 // Allow up to two consecutive newlines
                 if newline_count <= 2 {
                     cleaned.push(c);
                 }
            }
            cleaned.trim().to_string() // Trim final whitespace
        } else {
            final_content.trim().to_string() // Just trim final whitespace
        };

        HtmlConversionResult {
                markdown,
                links: self.links
        }
    }
}

pub fn html_to_markdown(html: &str, config: ConvertConfig) -> HtmlConversionResult {
    let dom = parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut html.as_bytes())
        .expect("Failed to parse HTML"); // Use expect for clearer error on parsing failure

    let mut formatter = MarkdownFormatter::new(config);

    // Find title tag specifically if metadata.title is still None
    if formatter.config.include_metadata && formatter.metadata.title.is_none() {
        find_title_tag(&dom.document, &mut formatter.metadata);
    }


    formatter.process_node(&dom.document);
    formatter.result()
}

// Helper function to find the <title> tag content
fn find_title_tag(handle: &Handle, metadata: &mut MetadataHandler) {
    match &handle.data {
        NodeData::Element { name, .. } if name.local.as_ref() == "title" => {
            // Extract text content from the title tag
            let mut title_content = String::new();
            extract_text(handle, &mut title_content);
            if !title_content.is_empty() && metadata.title.is_none() {
                metadata.title = Some(Cow::Owned(title_content.trim().to_string()));
            }
            return; // Stop searching once title is found
        }
        _ => {}
    }

    for child in handle.children.borrow().iter() {
         if metadata.title.is_some() { break; } // Stop if already found in a child
        find_title_tag(child, metadata);
    }
}

// Helper to extract text recursively
fn extract_text(handle: &Handle, buffer: &mut String) {
    match &handle.data {
        NodeData::Text { contents } => {
            buffer.push_str(contents.borrow().as_ref());
        }
        NodeData::Element { .. } => {
             for child in handle.children.borrow().iter() {
                 extract_text(child, buffer);
             }
        }
        _ => {} // Ignore comments, processing instructions, etc.
    }
}