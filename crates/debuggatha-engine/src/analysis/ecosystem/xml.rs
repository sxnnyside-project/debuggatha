//! The elements of an XML document as paths and text, enough to read a build manifest: nested
//! tags, attributes, comments, CDATA, processing instructions and self-closing tags. Entities are
//! not decoded.

/// Every element that holds text, as its path from the root (`project/artifactId`) and the text.
#[must_use]
pub(super) fn texts(source: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut stack: Vec<&str> = Vec::new();
    let mut rest = source;
    while let Some(open) = rest.find('<') {
        let text = rest[..open].trim();
        if !text.is_empty() && !stack.is_empty() {
            out.push((stack.join("/"), text.to_owned()));
        }
        rest = &rest[open..];
        if let Some(body) = rest.strip_prefix("<!--") {
            rest = body.find("-->").map_or("", |e| &body[e + 3..]);
        } else if let Some(body) = rest.strip_prefix("<![CDATA[") {
            let end = body.find("]]>").unwrap_or(body.len());
            if !stack.is_empty() {
                out.push((stack.join("/"), body[..end].to_owned()));
            }
            rest = body.get(end + 3..).unwrap_or("");
        } else if rest.starts_with("<?") || rest.starts_with("<!") {
            rest = rest.find('>').map_or("", |e| &rest[e + 1..]);
        } else {
            let Some(end) = rest.find('>') else { break };
            let tag = &rest[1..end];
            if let Some(name) = tag.strip_prefix('/') {
                if stack.last().is_some_and(|l| *l == name.trim()) {
                    stack.pop();
                }
            } else {
                let name = tag
                    .split(|c: char| c.is_whitespace() || c == '/')
                    .next()
                    .unwrap_or_default();
                if !tag.ends_with('/') {
                    stack.push(name);
                }
            }
            rest = &rest[end + 1..];
        }
    }
    out
}

/// The value of `attribute` on every tag called `tag` (`<ProjectReference Include="..\\b.csproj" />`).
#[must_use]
pub(super) fn attributes(source: &str, tag: &str, attribute: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = source;
    while let Some(open) = rest.find('<') {
        rest = &rest[open + 1..];
        if rest.starts_with(['!', '?', '/']) {
            continue;
        }
        let Some(end) = rest.find('>') else { break };
        let inner = rest[..end].trim_end_matches('/');
        let name = inner.split_whitespace().next().unwrap_or_default();
        if name == tag
            && let Some(value) = attribute_value(&inner[name.len()..], attribute)
        {
            out.push(value);
        }
        rest = &rest[end + 1..];
    }
    out
}

fn attribute_value(attrs: &str, attribute: &str) -> Option<String> {
    let mut rest = attrs.trim_start();
    while let Some((name, after)) = rest.split_once('=') {
        let after = after.trim_start();
        let quote = after.chars().next().filter(|c| matches!(c, '"' | '\''))?;
        let body = &after[1..];
        let close = body.find(quote)?;
        if name.trim() == attribute {
            return Some(body[..close].to_owned());
        }
        rest = body[close + 1..].trim_start();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{attributes, texts};

    #[test]
    fn attributes_of_a_tag_are_read_by_name() {
        let doc = "<Project><ItemGroup><ProjectReference Include=\"..\\b\\B.csproj\" Other='x' /><Reference Include=\"no\"/></ItemGroup></Project>";
        assert_eq!(
            attributes(doc, "ProjectReference", "Include"),
            vec!["..\\b\\B.csproj".to_owned()]
        );
    }

    #[test]
    fn nested_text_comments_and_self_closing_tags() {
        let doc = "<?xml version=\"1.0\"?><project a=\"1\"><!-- <x>no</x> --><parent><artifactId>up</artifactId></parent><artifactId>me</artifactId><empty/><d><![CDATA[a<b]]></d></project>";
        let found = texts(doc);
        assert_eq!(
            found,
            vec![
                ("project/parent/artifactId".to_owned(), "up".to_owned()),
                ("project/artifactId".to_owned(), "me".to_owned()),
                ("project/d".to_owned(), "a<b".to_owned()),
            ]
        );
    }
}
