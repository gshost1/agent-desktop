//! Live headless-Chrome check that a page's text reads one block at a time:
//! a paragraph is named by its text with its links kept as their own nodes,
//! and text never merges across the edge of a block-level element.
//! Ignored by default; run with
//! `cargo test -p agent-desktop-web --test live_inline_text -- --ignored`.

mod common;

use agent_desktop_core::{AccessibilityNode, NameSource};
use agent_desktop_web::{snapshot_page, wait_for_load};
use common::{connect_to, deadline, launch, open_tab, serve};

const PAGE: &str = r##"<!doctype html><html><head><title>Inline</title></head><body><main>
<p id="p1">Conceived in the late 1980s<sup id="r1"><a href="#n1"><span>[</span>1<span>]</span></a></sup> by <span id="s1"><a href="#ada">Ada Example</a></span>, at the <strong>institute</strong>.</p>
<ul><li><div>Title A</div><div>Subtitle B</div></li><li>Line one<br>Line two</li></ul>
<section><div>Div C</div><div>Div D</div></section>
<p><label for="f">Full name</label> <input id="f"></p>
</main></body></html>"##;

fn derived(node: &AccessibilityNode) -> Option<&str> {
    let descriptors = &node.presentation.descriptors;
    (descriptors.name_source == Some(NameSource::ContainedText))
        .then_some(descriptors.derived_name.as_deref())
        .flatten()
}

fn texts(node: &AccessibilityNode, out: &mut Vec<String>) {
    if let Some(text) = derived(node) {
        out.push(format!("{} ~{text}", node.role));
    } else if node.role == "statictext" {
        out.push(format!(
            "statictext {}",
            node.identity.name.as_deref().unwrap_or("")
        ));
    }
    for child in &node.children {
        texts(child, out);
    }
}

#[test]
#[ignore = "launches headless Chrome"]
fn a_page_reads_one_block_at_a_time_and_keeps_every_link() {
    let port = serve(PAGE.to_owned());
    let chrome = launch("inline-text");
    let mut connection = connect_to(&chrome);
    let tab = open_tab(&mut connection, &format!("http://127.0.0.1:{port}/"));
    wait_for_load(&mut connection, &tab, deadline()).unwrap();
    let snapshot = snapshot_page(&mut connection, &tab, deadline()).unwrap();

    let mut read = Vec::new();
    texts(&snapshot.tree, &mut read);
    assert_eq!(
        read,
        [
            "paragraph ~Conceived in the late 1980s[1] by Ada Example, at the institute.",
            "statictext Title A",
            "statictext Subtitle B",
            "statictext Line one Line two",
            "statictext Div C",
            "statictext Div D",
            "group ~Full name",
        ],
        "{:#?}",
        snapshot.tree
    );
    let paragraph = snapshot
        .preorder()
        .into_iter()
        .map(|(node, _)| node)
        .find(|node| node.role == "paragraph" && derived(node).is_some())
        .unwrap();
    let links: Vec<(&str, bool)> = paragraph
        .children
        .iter()
        .map(|link| {
            (
                link.identity.name.as_deref().unwrap_or(""),
                link.identity.retained_object.is_some(),
            )
        })
        .collect();
    assert_eq!(links, [("[1]", true), ("Ada Example", true)]);
    assert!(
        paragraph
            .children
            .iter()
            .all(|link| link.children.is_empty())
    );
}
