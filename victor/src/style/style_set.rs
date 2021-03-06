use crate::dom;
use crate::style::properties::{ComputedValues, LonghandDeclaration};
use crate::style::rules::{CssRule, RulesParser};
use crate::style::selectors::{self, Selector};
use cssparser::{Parser, ParserInput, RuleListParser};
use std::rc::Rc;

pub struct StyleSetBuilder(StyleSet);

pub struct StyleSet {
    rules: Vec<(Selector, Rc<Vec<LonghandDeclaration>>)>,
}

// XXX: if we ever replace Rc with Arc for style structs,
// replace thread_local! with lazy_static! here.
thread_local! {
    static USER_AGENT_STYLESHEET: StyleSet = {
        let mut builder = StyleSetBuilder::new();
        builder.add_stylesheet(include_str!("user_agent.css"));
        builder.finish()
    };
}

impl StyleSetBuilder {
    pub fn new() -> Self {
        StyleSetBuilder(StyleSet { rules: Vec::new() })
    }

    pub fn add_stylesheet(&mut self, css: &str) {
        let mut input = ParserInput::new(css);
        let mut parser = Parser::new(&mut input);
        for result in RuleListParser::new_for_stylesheet(&mut parser, RulesParser) {
            match result {
                Ok(CssRule::StyleRule {
                    selectors,
                    declarations,
                }) => {
                    for selector in selectors.0 {
                        self.0.rules.push((selector, declarations.clone()));
                    }
                }
                Err(_) => {
                    // FIXME: error reporting
                }
            }
        }
    }

    pub fn finish(mut self) -> StyleSet {
        // Sort stability preserves document order for rules of equal specificity
        self.0
            .rules
            .sort_by_key(|&(ref selector, _)| selector.specificity());
        self.0
    }
}

impl StyleSet {
    fn cascade_into(
        &self,
        document: &dom::Document,
        node: dom::NodeId,
        computed: &mut ComputedValues,
        inherited: &ComputedValues,
    ) {
        for &(ref selector, ref declarations) in &self.rules {
            if selectors::matches(selector, document, node) {
                for declaration in declarations.iter() {
                    declaration.cascade_into(computed, inherited)
                }
            }
        }
    }
}

pub(crate) fn cascade(
    author: &StyleSet,
    document: &dom::Document,
    node: dom::NodeId,
    parent_style: Option<&ComputedValues>,
) -> Rc<ComputedValues> {
    assert!(document[node].as_element().is_some());
    let initial = ComputedValues::initial();
    let inherited = parent_style.unwrap_or(&*initial);
    let mut computed = ComputedValues::new_inheriting_from(inherited, &*initial);
    USER_AGENT_STYLESHEET.with(|ua| ua.cascade_into(document, node, &mut computed, inherited));
    author.cascade_into(document, node, &mut computed, inherited);
    Rc::new(computed)
}
