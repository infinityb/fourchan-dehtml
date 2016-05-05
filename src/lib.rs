extern crate url;
extern crate html5ever;
extern crate tendril;
extern crate serde;
extern crate serde_json;

use std::default::Default;

use tendril::stream::TendrilSink;

use html5ever::{parse_document};
use html5ever::rcdom::RcDom;

pub mod dehtml;

#[derive(Clone, Copy, Debug)]
pub enum EncapKind {
    Quote,
    Spoiler,
    Ban,
}

impl EncapKind {
    pub fn as_str(&self) -> &'static str {
        use self::EncapKind as EK;
        match *self {
            EK::Quote => "quote",
            EK::Spoiler => "spoiler",
            EK::Ban => "ban",
        }
    }
}

impl serde::Serialize for EncapKind {
    #[inline]
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
        where S: serde::Serializer,
    {
        self.as_str().serialize(serializer)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LocalRef {
    post: u64,
}

#[derive(Clone, Debug)]
pub struct GlobalRef {
    board: String,
    target: GlobalRefTarget,
}

#[derive(Clone, Debug)]
pub enum GlobalRefTarget {
    Board,
    Catalog,
    Thread(u64),
    Post(u64, u64),
    Search(String),
}

impl GlobalRef {
    pub fn best_post(&self) -> Option<u64> {
        match self.target {
            GlobalRefTarget::Board => None,
            GlobalRefTarget::Catalog => None,
            GlobalRefTarget::Thread(thn) => Some(thn),
            GlobalRefTarget::Post(_, pn) => Some(pn),
            GlobalRefTarget::Search(_) => None,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Node {
    List(Vec<Node>),
    Encap(EncapKind, Box<Node>),
    LocalRef(LocalRef),
    GlobalRef(GlobalRef),
    Text(String),
    Anchor(String, Box<Node>),
}

impl serde::Serialize for Node {
    #[inline]
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
        where S: serde::Serializer,
    {
        use serde::Serialize;
        use self::Node as N;
        match *self {
            N::List(ref nodes) => {
                // ['list', [nodes]]
                // Ideally this would be [list,  *nodes] tho ...
                ("list", nodes).serialize(serializer)
            },
            N::Encap(kind, ref subnode) => {
                // [kind, subnode]
                (kind, subnode).serialize(serializer)
            },
            N::LocalRef(ref lref) => {
                // ['localref', post_id]
                ("localref", lref.post).serialize(serializer)
            },
            N::GlobalRef(GlobalRef { ref board, ref target }) => match *target {
                GlobalRefTarget::Board => {
                    ("globalref", "board", board).serialize(serializer)
                },
                GlobalRefTarget::Catalog => {
                    ("globalref", "catalog", board).serialize(serializer)
                },
                GlobalRefTarget::Thread(thread) => {
                    ("globalref", "thread", board, thread).serialize(serializer)
                },
                GlobalRefTarget::Post(thread, post) => {
                    ("globalref", "post", board, thread, post).serialize(serializer)
                },
                GlobalRefTarget::Search(ref term) => {
                    ("globalref", "search", board, term).serialize(serializer)
                }
            },
            N::Text(ref text) => {
                // ['text', text]
                ("text", text).serialize(serializer)
            },
            N::Anchor(ref url, ref child) => {
                ("anchor", url, child).serialize(serializer)
            }
        }
    }
}


fn list_optimize_helper(nodes: &[Node]) -> Vec<Node> {
    let mut out = Vec::new();
    for node in nodes.iter() {
        if let Node::List(ref nodes) = *node {
            out.extend(list_optimize_helper(nodes).into_iter());
        } else {
            out.push(node.clone());
        }   
    }
    out
}

fn merge_texts(nodes: &[Node]) -> Vec<Node> {
    let mut texts: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for node in nodes.iter() {
        if let Node::Text(ref st) = *node {
            texts.push(st.clone());
        } else {
            if texts.len() > 0 {
                out.push(Node::Text(texts.join("")));
                texts.clear();
            }
            out.push(node.clone())
        }
    }
    if texts.len() > 0 {
        out.push(Node::Text(texts.join("")));
        texts.clear();
    }
    out
}

fn list_optimize(nodes: &[Node]) -> Node {
    let out = list_optimize_helper(nodes);
    let out = merge_texts(&out);
    let mut out = list_optimize_helper(&out);
    if out.len() == 1 {
        out.pop().unwrap()
    } else {
        Node::List(out)
    }
}

impl Node {
    pub fn optimize(&self) -> Node {
        match *self {
            Node::List(ref children) => {
                let children: Vec<_> = children.iter().map(Node::optimize).collect();
                list_optimize(&children)
            }
            Node::Encap(ref kind, ref child) => {
                Node::Encap(*kind, Box::new(child.optimize()))
            },
            Node::LocalRef(ref lr) => Node::LocalRef(lr.clone()),
            Node::GlobalRef(ref gr) => Node::GlobalRef(gr.clone()),
            Node::Text(ref ss) => Node::Text(ss.clone()),
            Node::Anchor(ref u, ref ch) => Node::Anchor(u.clone(), Box::new(ch.optimize())),
        }
    }

    pub fn bbcode_fmt(&self, wri: &mut String) {
        use std::fmt::Write;
        use self::Node::*;
        match *self {
            List(ref nodes) => {
                for node in nodes.iter() {
                    node.bbcode_fmt(wri);
                }
            },
            Encap(EncapKind::Quote, ref node) => {
                write!(wri, "[quote]").unwrap();
                node.bbcode_fmt(wri);
                write!(wri, "[/quote]").unwrap();
            },
            Encap(EncapKind::Spoiler, ref node) => {
                write!(wri, "[spoiler]").unwrap();
                node.bbcode_fmt(wri);
                write!(wri, "[/spoiler]").unwrap();
            },
            Encap(EncapKind::Ban, ref node) => {
                write!(wri, "[ban]").unwrap();
                node.bbcode_fmt(wri);
                write!(wri, "[/ban]").unwrap();
            },
            LocalRef(ref po) => {
                write!(wri, ">>{}", po.post).unwrap();
            },
            GlobalRef(ref gr) => {
                match gr.best_post() {
                    Some(bp) => write!(wri, ">>>/{}/{}", gr.board, bp).unwrap(),
                    None => write!(wri, ">>>/{}/", gr.board).unwrap(),
                }
                
            },
            Text(ref text) => {
                write!(wri, "{}", text).unwrap();
            },
            Anchor(ref _url, ref text) => {
                text.bbcode_fmt(wri)
            },
        }
    }

    pub fn to_bbcode(&self) -> String {
        let mut output = String::new();
        self.bbcode_fmt(&mut output);
        output
    }
}

pub fn parse_html(buf: &str) -> Result<Node, dehtml::Error> {
    let dom: RcDom = parse_document(RcDom::default(), Default::default()).one(buf);
    let rv = dehtml::load_html(&[dom.document]);

    rv.map(|node| node.optimize())
}


#[cfg(test)]
mod tests {
    const COMMENTS: &'static str = include_str!("../comments");

    #[test]
    fn it_works0() {
        for part in COMMENTS.split('\u{00B6}') {
            println!("-----");
            match super::parse_html(part) {
                Ok(ast) => println!("as_bbcode = [[[{}]]]", ast.optimize().to_bbcode()),
                Err(err) => panic!("err = {:?} for doc {:?}", err, part),
            };
            println!("\n\n");
        }
        // panic!("zx");
    }

    #[test]
    fn it_works1() {
        use serde_json::ser::to_string_pretty;
        for part in COMMENTS.split('\u{00B6}') {
            println!("-----");
            match super::parse_html(part) {
                Ok(ast) => {
                    let optimised = ast.optimize();
                    let buf = to_string_pretty(&optimised).unwrap();
                    println!("as_bbcode = [[[{}]]]", buf);
                },
                Err(err) => panic!("err = {:?} for doc {:?}", err, part),
            };
            println!("\n\n");
        }
        panic!("zx");
    }
}
