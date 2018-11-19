use super::*;
use crate::dom;
use crate::style::values::*;
use crate::style::*;

impl<'arena> dom::Document<'arena> {
    pub fn render(&self) {
        let _ = self.box_tree();
    }

    fn box_tree(&self) -> BoxTreeRoot {
        let mut builder = StyleSetBuilder::new();
        self.parse_stylesheets(&mut builder);
        let author_styles = builder.finish();

        let root_element = self.root_element();
        let root_element_style = cascade(&author_styles, root_element, None);
        // If any, anonymous blocks wrapping inlines at the root level get initial styles,
        // they don’t have a parent element to inherit from.
        let initial_values = Rc::new(ComputedValues::new_inheriting_from(None));
        let mut builder = Builder::<BlockContainerBuilderExtra>::new(initial_values);
        builder.push_element(&author_styles, root_element, root_element_style);
        let (_, block) = builder.build();
        BlockFormattingContext(block)
    }
}

struct Builder<Extra> {
    style: Rc<ComputedValues>,
    consecutive_inline_levels: Vec<InlineLevel>,
    extra: Extra,
}

impl<Extra: Default + PushBlock> Builder<Extra> {
    fn new(style: Rc<ComputedValues>) -> Self {
        Self {
            style,
            consecutive_inline_levels: Vec::new(),
            extra: Extra::default(),
        }
    }
    fn push_child_elements(&mut self, author_styles: &StyleSet, parent_element: dom::NodeRef) {
        if let Some(first_child) = parent_element.first_child.get() {
            for child in first_child.self_and_next_siblings() {
                match &child.data {
                    dom::NodeData::Document
                    | dom::NodeData::Doctype { .. }
                    | dom::NodeData::Comment { .. }
                    | dom::NodeData::ProcessingInstruction { .. } => {}
                    dom::NodeData::Text { contents } => self.push_text(&contents.borrow()),
                    dom::NodeData::Element(_) => {
                        let style = cascade(author_styles, child, Some(&self.style));
                        self.push_element(author_styles, child, style)
                    }
                }
            }
        }
    }

    fn push_text(&mut self, text: &StrTendril) {
        if let Some(InlineLevel::Text(last_text)) = self.consecutive_inline_levels.last_mut() {
            last_text.push_tendril(&text)
        } else {
            self.consecutive_inline_levels
                .push(InlineLevel::Text(text.clone()))
        }
    }

    fn push_element(
        &mut self,
        author_styles: &StyleSet,
        element: dom::NodeRef,
        style: Rc<ComputedValues>,
    ) {
        match style.display.display {
            Display::None => {}
            Display::Other {
                outside: DisplayOutside::Inline,
                inside: DisplayInside::Flow,
            } => {
                let mut builder = Builder::<InlineBuilderExtra>::new(style);
                builder.push_child_elements(author_styles, element);
                for (previous_grand_children, block) in
                    builder.extra.self_fragments_split_by_block_levels
                {
                    if !previous_grand_children.is_empty() {
                        self.consecutive_inline_levels.push(InlineLevel::Inline {
                            style: Rc::clone(&builder.style),
                            children: previous_grand_children,
                        })
                    }
                    Extra::push_block(self, block)
                }
                let grand_children = builder.consecutive_inline_levels;
                if !grand_children.is_empty() {
                    self.consecutive_inline_levels.push(InlineLevel::Inline {
                        style: builder.style,
                        children: grand_children,
                    })
                }
            }
            Display::Other {
                outside: DisplayOutside::Block,
                inside: DisplayInside::Flow,
            } => {
                let mut builder = Builder::<BlockContainerBuilderExtra>::new(style);
                builder.push_child_elements(author_styles, element);
                let (style, contents) = builder.build();
                Extra::push_block(
                    self,
                    BlockLevel::SameFormattingContextBlock { style, contents },
                )
            }
        }
    }
}

trait PushBlock: Sized {
    fn push_block(builder: &mut Builder<Self>, block: BlockLevel);
}

#[derive(Default)]
struct InlineBuilderExtra {
    self_fragments_split_by_block_levels: Vec<(Vec<InlineLevel>, BlockLevel)>,
}

impl PushBlock for InlineBuilderExtra {
    fn push_block(builder: &mut Builder<Self>, block: BlockLevel) {
        builder
            .extra
            .self_fragments_split_by_block_levels
            .push((builder.consecutive_inline_levels.take(), block))
    }
}

#[derive(Default)]
struct BlockContainerBuilderExtra {
    block_levels: Vec<BlockLevel>,
}

impl PushBlock for BlockContainerBuilderExtra {
    fn push_block(builder: &mut Builder<Self>, block: BlockLevel) {
        if !builder.consecutive_inline_levels.is_empty() {
            builder.wrap_inlines_in_anonymous_block();
        }
        builder.extra.block_levels.push(block)
    }
}
impl Builder<BlockContainerBuilderExtra> {
    fn wrap_inlines_in_anonymous_block(&mut self) {
        self.extra
            .block_levels
            .push(BlockLevel::SameFormattingContextBlock {
                style: ComputedValues::anonymous_inheriting_from(&self.style),
                contents: BlockContainer::InlineFormattingContext(
                    self.consecutive_inline_levels.take(),
                ),
            });
    }

    fn build(mut self) -> (Rc<ComputedValues>, BlockContainer) {
        if !self.consecutive_inline_levels.is_empty() {
            if self.extra.block_levels.is_empty() {
                return (
                    self.style,
                    BlockContainer::InlineFormattingContext(self.consecutive_inline_levels),
                )
            }
            self.wrap_inlines_in_anonymous_block()
        }
        (
            self.style,
            BlockContainer::BlockLevels(self.extra.block_levels),
        )
    }
}

trait Take {
    fn take(&mut self) -> Self;
}

impl<T> Take for Vec<T> {
    fn take(&mut self) -> Self {
        std::mem::replace(self, Vec::new())
    }
}
