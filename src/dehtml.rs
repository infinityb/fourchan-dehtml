use std::default::Default;
use std::collections::HashSet;

use url;
use html5ever::Attribute;
use html5ever::rcdom::{Document, Doctype, Text, Comment, Element, Handle};

use super::Node as SNode;
use super::{LocalRef, GlobalRef, GlobalRefTarget, EncapKind};

#[derive(Clone, Debug)]
pub enum Node {
    List(Vec<Node>),
    Encap(EncapKind, Box<Node>),
    LocalRef(LocalRef),
    GlobalRef(GlobalRef),
    Text(String),
    LineBreak,
    WordBreakOpportunity,
    Anchor(String, Box<Node>),
}

#[derive(Debug)]
pub enum Error {
    NoBody,
    BadAnchor,
    BadRef,
    BadDocument,
    BadDeadlink(String),
    Unhandled,
    UnknownClassSet(HashSet<String>),
}

impl Node {
    pub fn to_super(&self) -> super::Node {
        match *self {
            Node::List(ref nodes) => {
                SNode::List(nodes.iter().map(|x| x.to_super()).collect())
            },
            Node::Encap(kind, ref n) => {
                SNode::Encap(kind, Box::new(n.to_super()))
            },
            Node::LocalRef(ref lr) => SNode::LocalRef(lr.clone()),
            Node::GlobalRef(ref gr) => SNode::GlobalRef(gr.clone()),
            Node::Text(ref buf) => SNode::Text(buf.to_string()),
            Node::LineBreak => SNode::Text("\n".to_string()),
            Node::WordBreakOpportunity => SNode::Text(String::new()),
            Node::Anchor(ref url, ref child) => SNode::Anchor(url.clone(), Box::new(child.to_super())),
        }
    }
}

fn find_body(handles: &[Handle]) -> Option<&Handle> {
    use html5ever::rcdom::ElementEnum::Normal;

    for handle in handles.iter() {
        let node = handle.borrow();
        match node.node {
            Element(ref name, Normal, _) if name.local.as_ref() == "body" => {
                return Some(handle);
            }
            _ => (),
        }
    }
    None
}

fn identify(handles: &[Handle]) -> Result<Node, Error> {
    let mut output = Vec::new();
    for handle in handles.iter() {
        if let Some(node) = try!(identify_handle(handle)) {
            output.push(node);
        }
    }
    Ok(Node::List(output))
}

struct Span {
    classes: HashSet<String>,
}

impl Span {
    pub fn from_attrs(attrs: &[Attribute]) -> Self {
        let mut classes: HashSet<String> = Default::default();
        for attr in attrs.iter() {
            let key = attr.name.local.as_ref();
            if "class" == key {
                classes.extend(attr.value.as_ref().split(' ').map(|x| x.to_string()));
            }
        }
        Span { classes: classes }
    }
}

fn identify_span_deadlink(text: &str) -> Result<Node, Error> {
    let err = || { Error::BadDeadlink(text.to_string()) };
    let parts: Vec<&str> = text.splitn(3, "/").collect();

    if parts.len() == 1 {
        let post_id = try!(text[2..].parse().map_err(|_| err()));
        return Ok(Node::LocalRef(LocalRef { post: post_id }))
    }
    if parts.len() < 2 || 3 < parts.len() {
        return Err(Error::BadDeadlink(text.to_string()));
    }
    if parts[0] != ">>>" && parts[0] != ">>" {
        return Err(Error::BadDeadlink(text.to_string()));
    }
    if parts.len() == 3 {
        let post_id = try!(parts[2].parse().map_err(|_| err()));
        Ok(Node::GlobalRef(GlobalRef {
            board: parts[1].to_string(),
            target: GlobalRefTarget::Thread(post_id),
        }))
    } else {
        let post_id = try!(parts[2].parse().map_err(|_| err()));
        Ok(Node::LocalRef(LocalRef { post: post_id }))
    }
}

fn identify_span(attrs: &[Attribute], children: &[Handle]) -> Result<Node, Error> {
    let span = Span::from_attrs(attrs);

    if span.classes.contains("quote") {
        identify(children).map(|n|
            Node::Encap(EncapKind::Quote, Box::new(n)))
    } else if span.classes.contains("deadlink") {
        let child_text_res = children.get(0).ok_or(Error::Unhandled).and_then(|handle| {
            let node = handle.borrow();
            match node.node {
                Text(ref text) => Ok(text.to_string()),
                _ => Err(Error::Unhandled),
            }
        });
        let child_text = try!(child_text_res);
        identify_span_deadlink(&child_text)
    } else {
        println!("span.classes = {:?}", span.classes);
        Err(Error::UnknownClassSet(span.classes))
    }
}

struct Anchor {
    classes: HashSet<String>,
    href: String,
}

impl Anchor {
    pub fn from_attrs(attrs: &[Attribute]) -> Option<Self> {
        let mut classes: HashSet<String> = Default::default();
        let mut href: Option<String> = None;
        for attr in attrs.iter() {
            let key = attr.name.local.as_ref();
            if "class" == key {
                classes.extend(attr.value.as_ref().split(' ').map(|x| x.to_string()));
            }
            if "href" == key {
                href = Some(attr.value.as_ref().to_string());
            }
        }
        href.map(|href| Anchor {
            classes: classes,
            href: href,
        })
    }
}

fn identify_ref(anchor: Anchor) -> Result<Node, Error> {
    if anchor.href.starts_with("//") {
        let base = url::Url::parse("https://example.com").unwrap();
        let mut parser = url::UrlParser::new();
        parser.base_url(&base);

        let url = try!(parser.parse(&anchor.href).map_err(|err| {
            println!("err = {:?}", err);
            Error::BadRef
        }));
        if anchor.href.len() % 5 < 5 {
            // fix the FIX ME 
            return Err(Error::Unhandled);
        }
        return Ok(Node::Anchor(anchor.href.into(), Box::new(Node::Text("FIX ME".into()))))
    }
    if anchor.href.starts_with("#") {
        let post = try!(anchor.href[2..].parse().map_err(|_| Error::BadRef));
        return Ok(Node::LocalRef(LocalRef { post: post }))
    }

    let (path, midl, frag) = try!(url::parse_path(&anchor.href).map_err(|_| Error::BadRef));
    if path.len() < 2 {
        return Err(Error::BadRef);
    }
    let target = if path[1] == "" {
        // no thread
        GlobalRefTarget::Board
    } else if path[1] == "catalog" {
        let frag = frag.unwrap_or_else(String::new);
        if frag.len() == 0 {
            GlobalRefTarget::Catalog
        } else if frag.starts_with("s=") {
            GlobalRefTarget::Search(frag[2..].to_string())
        } else {
            return Err(Error::BadRef);
        }
    } else if path[1] == "thread" {
        let thread_num = match path.get(2) {
            Some(ref part) if 1 < part.len() => {
                let thread_num = try!(part.parse().map_err(|_| Error::BadRef));
                Some(thread_num)
            },
            Some(_empty) => None,
            _ => None,
        };
        let post_num = match frag {
            Some(ref frag) if 1 < frag.len() => {
                Some(try!(frag[1..].parse().map_err(|_| Error::BadRef)))
            },
            Some(_) => None,
            None => None,
        };
        match (thread_num, post_num) {
            (Some(thn), None) => GlobalRefTarget::Thread(thn),
            (Some(thn), Some(pn)) => GlobalRefTarget::Post(thn, pn),
            _ => return Err(Error::BadRef),
        }
    } else {
        return Err(Error::BadRef);
    };
    Ok(Node::GlobalRef(GlobalRef {
        board: path[0].clone(),
        target: target,
    }))
}

fn identify_anchor(attrs: &[Attribute], children: &[Handle]) -> Result<Node, Error> {
    let anchor = try!(Anchor::from_attrs(attrs).ok_or(Error::BadAnchor));
    if anchor.classes.contains("quotelink") {
        identify_ref(anchor)
    } else {
        identify(children)
    }
}

fn identify_strong(attrs: &[Attribute], children: &[Handle]) -> Result<Node, Error> {
    // <strong style=\"color: red;\">(USER WAS BANNED FOR THIS POST)</strong>
    for attr in attrs.iter() {
        if attr.name.local.as_ref() == "style" {
            if attr.value.as_ref() == "color: red;" {
                let children = try!(identify(children));
                return Ok(Node::Encap(EncapKind::Ban, Box::new(children)));
            }
        }
    }
    Err(Error::Unhandled)
}

fn identify_document(handle: &Handle) -> Result<Option<Node>, Error> {
    let node = handle.borrow();
    let html = try!(node.children.get(0).ok_or(Error::BadDocument));
    let html = html.borrow();

    find_body(&html.children)
        .ok_or(Error::NoBody)
        .and_then(identify_handle)
}

fn identify_handle(handle: &Handle) -> Result<Option<Node>, Error> {
    use html5ever::rcdom::ElementEnum::Normal;
    let node = handle.borrow();

    match node.node {
        Document => identify_document(handle),
        Doctype(_, _, _) => Ok(None),
        Text(ref text) => Ok(Some(Node::Text(From::from(text)))),
        Comment(_) => Ok(None),
        Element(ref name, Normal, _) if name.local.as_ref() == "body" => {
            identify(&node.children).map(Some)
        },
        Element(ref name, Normal, _) if name.local.as_ref() == "wbr" => {
            Ok(Some(Node::WordBreakOpportunity))
        },
        Element(ref name, Normal, _) if name.local.as_ref() == "br" => {
            Ok(Some(Node::LineBreak))
        },
        Element(ref name, Normal, ref attrs) if name.local.as_ref() == "span" => {
            identify_span(&attrs, &node.children).map(Some)
        },
        Element(ref name, Normal, _) if name.local.as_ref() == "s" => {
            identify(&node.children)
                .map(|n| Some(Node::Encap(EncapKind::Spoiler, Box::new(n))))
        },
        Element(ref name, Normal, ref attrs) if name.local.as_ref() == "a" => {
            identify_anchor(&attrs, &node.children).map(Some)
        },
        Element(ref name, Normal, ref attrs) if name.local.as_ref() == "strong" => {
            identify_strong(&attrs, &node.children).map(Some)
        },
        Element(_, _, _) => Err(Error::Unhandled),
    }
}

pub fn load_html(node: &[Handle]) -> Result<super::Node, Error> {
    identify(node).map(|n| n.to_super())
}
