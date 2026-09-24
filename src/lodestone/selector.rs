//! Compiled lodestone-css-selectors entries and typed extraction helpers.
//!
//! A selector file entry is `{ "selector", "attribute"?, "regex"? }`. [`Sel`]
//! compiles the CSS selector and regex while it is deserialized, so a broken
//! vendored file fails at boot, not on a request. Upstream regexes use
//! Python-style named groups (`(?P<Name>…)`), which the `regex` crate supports.

use std::{fmt, str::FromStr};

use chrono::{DateTime, Utc};
use regex::{CaptureLocations, Regex};
use scraper::{element_ref::Select, node::Node, ElementRef, Html, Selector};
use serde::{de, Deserialize, Deserializer};
use url::Url;

use super::pages::common::{Paged, Pagination};

/// One compiled selector entry.
pub struct Sel {
    source: Box<str>,
    css: Selector,
    attribute: Option<Box<str>>,
    regex: Option<Regex>,
}

impl fmt::Debug for Sel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Sel").field(&self.source).finish()
    }
}

impl<'de> Deserialize<'de> for Sel {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // `type`, `multiple` and friends are descriptive only; the Rust types
        // decide how a value is read.
        #[derive(Deserialize)]
        struct Raw {
            selector: String,
            attribute: Option<String>,
            regex: Option<String>,
        }

        let raw = Raw::deserialize(deserializer)?;
        let css = Selector::parse(&raw.selector)
            .map_err(|e| de::Error::custom(format!("invalid selector `{}`: {e}", raw.selector)))?;
        let regex = raw
            .regex
            .map(|r| {
                Regex::new(&r).map_err(|e| de::Error::custom(format!("invalid regex `{r}`: {e}")))
            })
            .transpose()?;
        Ok(Self {
            source: raw.selector.into(),
            css,
            attribute: raw.attribute.map(Into::into),
            regex,
        })
    }
}

impl Sel {
    #[must_use]
    pub fn first<'a>(&self, scope: ElementRef<'a>) -> Option<ElementRef<'a>> {
        scope.select(&self.css).next()
    }

    #[must_use]
    pub fn all<'a, 's>(&'s self, scope: ElementRef<'a>) -> Select<'a, 's> {
        scope.select(&self.css)
    }

    #[must_use]
    pub fn exists(&self, scope: ElementRef<'_>) -> bool {
        self.first(scope).is_some()
    }

    /// The configured attribute, or else the element's text (`<br>` becomes a
    /// newline). Trimmed; empty values are `None`. Ignores `regex`.
    #[must_use]
    pub fn text(&self, scope: ElementRef<'_>) -> Option<String> {
        let el = self.first(scope)?;
        match &self.attribute {
            Some(attr) => non_empty(el.value().attr(attr)?),
            None => non_empty(&element_text(el)),
        }
    }

    /// The element's text even when the entry is configured to read an
    /// attribute (e.g. a link whose file entry targets its `href`).
    #[must_use]
    pub fn inner_text(&self, scope: ElementRef<'_>) -> Option<String> {
        non_empty(&element_text(self.first(scope)?))
    }

    /// A specific attribute of the matched element, regardless of the
    /// configured one (e.g. the `href` of a link the file only reads text from).
    #[must_use]
    pub fn attr(&self, scope: ElementRef<'_>, name: &str) -> Option<String> {
        non_empty(self.first(scope)?.value().attr(name)?)
    }

    /// [`Self::text`] parsed as an absolute URL.
    #[must_use]
    pub fn url(&self, scope: ElementRef<'_>) -> Option<Url> {
        Url::parse(&self.text(scope)?).ok()
    }

    /// [`Self::text`] parsed as a number, ignoring thousands separators.
    #[must_use]
    pub fn num<T: FromStr>(&self, scope: ElementRef<'_>) -> Option<T> {
        parse_num(&self.text(scope)?)
    }

    /// The first run of digits inside [`Self::text`] (e.g. `Item Level 710`).
    #[must_use]
    pub fn digits<T: FromStr>(&self, scope: ElementRef<'_>) -> Option<T> {
        let text = self.text(scope)?;
        let start = text.find(|c: char| c.is_ascii_digit())?;
        let run: String = text[start..]
            .chars()
            .take_while(|c| c.is_ascii_digit() || matches!(c, ',' | '.'))
            .collect();
        parse_num(&run)
    }

    /// Runs the configured regex.
    ///
    /// With an `attribute`, the regex runs on the attribute value. Otherwise it
    /// runs on the element's inner HTML, because several upstream regexes match
    /// markup such as `<br/>`. html5ever serialises void tags as `<br>`, so
    /// they are restored to `<br/>` first. If that does not match, the regex is
    /// retried on the decoded text (e.g. `Name <Rank>` is `&lt;Rank&gt;` in HTML).
    #[must_use]
    pub fn captures(&self, scope: ElementRef<'_>) -> Option<Caps<'_>> {
        let re = self.regex.as_ref()?;
        let el = self.first(scope)?;
        let mut locs = re.capture_locations();
        let mut attempt = |hay: String| re.captures_read(&mut locs, &hay).is_some().then_some(hay);
        let hay = match &self.attribute {
            Some(attr) => attempt(el.value().attr(attr)?.to_owned()),
            None => attempt(el.inner_html().replace("<br>", "<br/>"))
                .or_else(|| attempt(element_text(el))),
        }?;
        Some(Caps { re, hay, locs })
    }

    /// One named capture group; see [`Self::captures`].
    #[must_use]
    pub fn capture(&self, scope: ElementRef<'_>, group: &str) -> Option<String> {
        self.captures(scope)?.get(group)
    }

    /// One named capture group parsed as a number.
    #[must_use]
    pub fn capture_num<T: FromStr>(&self, scope: ElementRef<'_>, group: &str) -> Option<T> {
        self.captures(scope)?.num(group)
    }

    /// A `ldst_strftime(<unix seconds>, …)` script's timestamp.
    #[must_use]
    pub fn timestamp(&self, scope: ElementRef<'_>) -> Option<DateTime<Utc>> {
        DateTime::from_timestamp(self.capture_num(scope, "Timestamp")?, 0)
    }
}

/// Owned result of a regex match on a [`Sel`].
pub struct Caps<'s> {
    re: &'s Regex,
    hay: String,
    locs: CaptureLocations,
}

impl Caps<'_> {
    /// A named group, HTML-entity-decoded and trimmed; empty is `None`.
    #[must_use]
    pub fn get(&self, group: &str) -> Option<String> {
        let idx = self.re.capture_names().position(|n| n == Some(group))?;
        let (start, end) = self.locs.get(idx)?;
        non_empty(&html_escape::decode_html_entities(&self.hay[start..end]))
    }

    #[must_use]
    pub fn num<T: FromStr>(&self, group: &str) -> Option<T> {
        parse_num(&self.get(group)?)
    }
}

/// A `ROOT` + entry-fields group: `ROOT` matches each list item, and the
/// entry selectors are evaluated inside it.
#[derive(Debug, Deserialize)]
pub struct ListSel<E> {
    #[serde(rename = "ROOT")]
    pub root: Sel,
    #[serde(flatten)]
    pub entry: E,
}

impl<E> ListSel<E> {
    /// Parses every matched item with `f`, dropping items it rejects.
    pub fn parse<T>(
        &self,
        scope: ElementRef<'_>,
        f: impl Fn(&E, ElementRef<'_>) -> Option<T>,
    ) -> Vec<T> {
        self.root
            .all(scope)
            .filter_map(|el| f(&self.entry, el))
            .collect()
    }
}

/// A paginated list file (`ROOT`, `ENTRY`, `PAGE_INFO`, …).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct PagedSel<E> {
    /// Narrows the document before anything else is evaluated.
    pub root: Option<Sel>,
    pub entry: ListSel<E>,
    pub page_info: Sel,
}

impl<E> PagedSel<E> {
    pub fn parse<T>(&self, doc: &Html, f: impl Fn(&E, ElementRef<'_>) -> Option<T>) -> Paged<T> {
        let root = doc.root_element();
        let scope = self.root.as_ref().map_or(Some(root), |sel| sel.first(root));
        let Some(scope) = scope else {
            return Paged::empty();
        };
        let items = self.entry.parse(scope, f);
        let caps = self.page_info.captures(scope);
        let page = caps
            .as_ref()
            .and_then(|c| c.num("CurrentPage"))
            .unwrap_or(1);
        let total = caps
            .as_ref()
            .and_then(|c| c.num("NumPages"))
            .unwrap_or_else(|| u16::from(!items.is_empty()));
        Paged {
            items,
            pagination: Pagination::new(page, total),
        }
    }
}

/// Text of an element and its descendants, with `<br>` as `\n`.
fn element_text(el: ElementRef<'_>) -> String {
    let mut out = String::new();
    for node in el.descendants() {
        match node.value() {
            Node::Text(text) => out.push_str(text),
            Node::Element(e) if e.name() == "br" => out.push('\n'),
            _ => {}
        }
    }
    out
}

fn non_empty(s: &str) -> Option<String> {
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_owned())
}

/// Parses a number, ignoring whitespace and `,`/`.` thousands separators
/// (all numeric Lodestone fields are integers). `-`, `--` and friends fail.
fn parse_num<T: FromStr>(s: &str) -> Option<T> {
    let cleaned: String = s
        .chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, ',' | '.'))
        .collect();
    cleaned.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sel(json: &str) -> Sel {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn invalid_css_or_regex_fails_to_load() {
        assert!(serde_json::from_str::<Sel>(r#"{"selector":"div >> >"}"#).is_err());
        assert!(serde_json::from_str::<Sel>(r#"{"selector":"div","regex":"(?P<x"}"#).is_err());
    }

    #[test]
    fn extracts_text_attr_numbers_and_captures() {
        let doc = Html::parse_document(
            r#"<p class="a">Raen<br>Raen / &#9792;</p>
               <p class="w"> Gilgamesh [Aether] </p>
               <p class="n">1,234,567 / 2,000,000</p>
               <p class="gc">Maelstrom &lt;Allied&gt;</p>
               <img class="i" src="https://img.example/x.png">
               <div class="il">Item Level 710</div>"#,
        );
        let root = doc.root_element();

        let rcg = sel(
            r#"{"selector":".a","regex":"(?P<Race>.*)<br\\/?>(?P<Tribe>.*) \\/ (?P<Gender>.)"}"#,
        );
        let caps = rcg.captures(root).unwrap();
        assert_eq!(caps.get("Race").as_deref(), Some("Raen"));
        assert_eq!(caps.get("Gender").as_deref(), Some("♀"));
        assert_eq!(rcg.text(root).as_deref(), Some("Raen\nRaen / ♀"));

        let world = sel(r#"{"selector":".w","regex":"(?P<World>\\w*)\\s+\\[(?P<DC>\\w*)\\]"}"#);
        assert_eq!(world.capture(root, "DC").as_deref(), Some("Aether"));

        let exp = sel(r#"{"selector":".n","regex":"(?P<Cur>\\S+) \\/ (?P<Max>\\S+)"}"#);
        assert_eq!(exp.capture_num::<u64>(root, "Cur"), Some(1_234_567));

        let gc = sel(r#"{"selector":".gc","regex":"(?P<Name>\\w.*)<(?P<Rank>\\w*)>"}"#);
        assert_eq!(gc.capture(root, "Rank").as_deref(), Some("Allied"));

        let img = sel(r#"{"selector":".i","attribute":"src"}"#);
        assert_eq!(img.url(root).unwrap().host_str(), Some("img.example"));

        assert_eq!(sel(r#"{"selector":".il"}"#).digits::<u16>(root), Some(710));
        assert_eq!(sel(r#"{"selector":".missing"}"#).text(root), None);
    }
}
