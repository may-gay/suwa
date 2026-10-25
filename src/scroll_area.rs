use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer, text};
use iced::widget::scrollable::{self, Direction, Scrollbar};
use iced::{Element, Event, Length, Rectangle, Size, Theme, Vector};
use std::{
    cell::Cell,
    rc::Rc,
    time::{Duration, Instant},
};

const HIDE_AFTER: Duration = Duration::from_millis(800);

pub struct ScrollArea<'a, Message, Renderer = iced::Renderer>
where
    Renderer: text::Renderer,
{
    inner: scrollable::Scrollable<'a, Message, Theme, Renderer>,
    activity: Rc<Cell<Option<Instant>>>,
    wheel_horizontal: bool,
    position: Rc<Cell<Vector>>,
}

#[derive(Default)]
struct State {
    last_activity: Option<Instant>,
    position: Vector,
}

fn compact(bar: Scrollbar) -> Scrollbar {
    if bar == Scrollbar::hidden() {
        bar
    } else {
        bar.width(8).scroller_width(4).margin(2)
    }
}

pub fn scrollable<'a, Message: 'a, Renderer: text::Renderer + 'a>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> ScrollArea<'a, Message, Renderer> {
    let activity = Rc::new(Cell::new(None::<Instant>));
    let visible = activity.clone();
    ScrollArea {
        inner: scrollable::Scrollable::new(content)
            .direction(Direction::Vertical(compact(Scrollbar::new())))
            .style(move |theme, status| {
                crate::ui::scroll_style(
                    theme,
                    status,
                    visible.get().is_some_and(|at| at.elapsed() < HIDE_AFTER),
                )
            }),
        activity,
        wheel_horizontal: false,
        position: Rc::new(Cell::new(Vector::ZERO)),
    }
}

impl<'a, Message, Renderer: text::Renderer> ScrollArea<'a, Message, Renderer> {
    pub fn id(mut self, id: impl Into<iced::widget::Id>) -> Self {
        self.inner = self.inner.id(id);
        self
    }

    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.inner = self.inner.width(width);
        self
    }

    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.inner = self.inner.height(height);
        self
    }

    pub fn on_scroll(mut self, f: impl Fn(scrollable::Viewport) -> Message + 'a) -> Self {
        let activity = self.activity.clone();
        let position = self.position.clone();
        self.inner = self.inner.on_scroll(move |viewport| {
            let offset = viewport.absolute_offset();
            let offset = Vector::new(offset.x, offset.y);
            if position.replace(offset) != offset {
                activity.set(Some(Instant::now()));
            }
            f(viewport)
        });
        self
    }

    pub fn direction(mut self, direction: Direction) -> Self {
        self.inner = self.inner.direction(match direction {
            Direction::Vertical(bar) => Direction::Vertical(compact(bar)),
            Direction::Horizontal(bar) => Direction::Horizontal(compact(bar)),
            Direction::Both {
                vertical,
                horizontal,
            } => Direction::Both {
                vertical: compact(vertical),
                horizontal: compact(horizontal),
            },
        });
        self
    }

    pub fn wheel_horizontal(mut self) -> Self {
        self.wheel_horizontal = true;
        self
    }
}

fn horizontal_wheel(delta: mouse::ScrollDelta) -> mouse::ScrollDelta {
    let (x, y, scale) = match delta {
        mouse::ScrollDelta::Lines { x, y } => (x, y, 60.0),
        mouse::ScrollDelta::Pixels { x, y } => (x, y, 1.0),
    };
    mouse::ScrollDelta::Pixels {
        x: if x.abs() > y.abs() { x } else { y } * scale,
        y: 0.0,
    }
}

impl<Message, Renderer: text::Renderer> Widget<Message, Theme, Renderer>
    for ScrollArea<'_, Message, Renderer>
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(
            &self.inner as &dyn Widget<Message, Theme, Renderer>,
        )]
    }

    fn diff(&self, tree: &mut Tree) {
        self.activity
            .set(tree.state.downcast_ref::<State>().last_activity);
        self.position
            .set(tree.state.downcast_ref::<State>().position);
        self.inner.diff(&mut tree.children[0]);
    }

    fn size(&self) -> Size<Length> {
        self.inner.size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.inner.layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.inner
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let over = cursor.is_over(layout.bounds());
        if over
            && matches!(
                event,
                Event::Mouse(mouse::Event::WheelScrolled { .. })
                    | Event::Touch(iced::touch::Event::FingerMoved { .. })
            )
        {
            self.activity.set(Some(Instant::now()));
            shell.request_redraw();
        }
        let horizontal = if self.wheel_horizontal && over {
            match event {
                Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                    Some(Event::Mouse(mouse::Event::WheelScrolled {
                        delta: horizontal_wheel(*delta),
                    }))
                }
                _ => None,
            }
        } else {
            None
        };
        self.inner.update(
            &mut tree.children[0],
            horizontal.as_ref().unwrap_or(event),
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
        let state = tree.state.downcast_mut::<State>();
        state.last_activity = self.activity.get();
        state.position = self.position.get();
        if let Some(at) = state.last_activity {
            let deadline = at + HIDE_AFTER;
            if Instant::now() < deadline {
                shell.request_redraw_at(deadline);
            } else {
                state.last_activity = None;
                self.activity.set(None);
            }
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.inner
            .mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.inner.draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.inner.overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message: 'a, Renderer: text::Renderer + 'a> From<ScrollArea<'a, Message, Renderer>>
    for Element<'a, Message, Theme, Renderer>
{
    fn from(area: ScrollArea<'a, Message, Renderer>) -> Self {
        Element::new(area)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::{Point, widget::Space, window};

    struct Harness {
        area: ScrollArea<'static, scrollable::Viewport, ()>,
        tree: Tree,
        node: layout::Node,
    }

    impl Harness {
        fn new(horizontal: bool) -> Self {
            let content = Space::new()
                .width(if horizontal { 900 } else { 240 })
                .height(if horizontal { 40 } else { 900 });
            let mut area = scrollable(content).width(240).height(40).on_scroll(|v| v);
            if horizontal {
                area = area
                    .direction(Direction::Horizontal(Scrollbar::hidden()))
                    .wheel_horizontal();
            }
            let mut tree = Tree::new(&area as &dyn Widget<scrollable::Viewport, Theme, ()>);
            let node = area.layout(
                &mut tree,
                &(),
                &layout::Limits::new(Size::ZERO, Size::new(240.0, 40.0)),
            );
            Self { area, tree, node }
        }

        fn event(
            &mut self,
            event: Event,
            cursor: Point,
        ) -> (Vec<scrollable::Viewport>, window::RedrawRequest) {
            let mut messages = Vec::new();
            let mut shell = Shell::new(&mut messages);
            self.area.update(
                &mut self.tree,
                &event,
                Layout::new(&self.node),
                mouse::Cursor::Available(cursor),
                &(),
                &mut iced::advanced::clipboard::Null,
                &mut shell,
                &Rectangle::with_size(Size::new(240.0, 40.0)),
            );
            let redraw = shell.redraw_request();
            (messages, redraw)
        }
    }

    #[test]
    fn category_strip_accepts_wheel_and_touchpad_only_inside_its_bounds() {
        let mut h = Harness::new(true);
        let wheel = |delta| Event::Mouse(mouse::Event::WheelScrolled { delta });
        let inside = Point::new(50.0, 20.0);
        let outside = Point::new(50.0, 60.0);
        let lines = mouse::ScrollDelta::Lines { x: 0.0, y: -1.0 };
        assert!(h.event(wheel(lines), outside).0.is_empty());
        assert!(h.area.activity.get().is_none());

        let (messages, _) = h.event(wheel(lines), inside);
        assert_eq!(
            messages.last().unwrap().absolute_offset(),
            scrollable::AbsoluteOffset { x: 60.0, y: 0.0 }
        );
        assert!(h.event(wheel(lines), outside).0.is_empty());

        let (messages, _) = h.event(
            wheel(mouse::ScrollDelta::Pixels { x: -12.5, y: -1.0 }),
            inside,
        );
        assert_eq!(messages.last().unwrap().absolute_offset().x, 72.5);
        let (messages, _) = h.event(wheel(mouse::ScrollDelta::Pixels { x: 0.0, y: 8.0 }), inside);
        assert_eq!(messages.last().unwrap().absolute_offset().x, 64.5);

        let _ = h.event(
            Event::Keyboard(iced::keyboard::Event::ModifiersChanged(
                iced::keyboard::Modifiers::SHIFT,
            )),
            inside,
        );
        let (messages, _) = h.event(wheel(lines), inside);
        assert_eq!(messages.last().unwrap().absolute_offset().x, 124.5);
    }

    #[test]
    fn scroll_activity_survives_rebuilds_and_expires_without_mouse_movement() {
        let mut h = Harness::new(false);
        let outside = Point::new(250.0, 60.0);
        let _ = h.event(
            Event::Window(window::Event::RedrawRequested(Instant::now())),
            outside,
        );
        assert!(h.area.activity.get().is_none());
        let _ = h.event(
            Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Lines { x: 0.0, y: -1.0 },
            }),
            Point::new(50.0, 20.0),
        );
        let active = h.area.activity.get().unwrap();

        h.area = Harness::new(false).area;
        h.area.diff(&mut h.tree);
        assert_eq!(h.area.activity.get(), Some(active));
        let (_, redraw) = h.event(
            Event::Window(window::Event::RedrawRequested(Instant::now())),
            outside,
        );
        assert_eq!(redraw, window::RedrawRequest::At(active + HIDE_AFTER));
        assert!(Harness::new(false).area.activity.get().is_none());

        let expired = Instant::now() - HIDE_AFTER - Duration::from_millis(1);
        h.area.activity.set(Some(expired));
        h.tree.state.downcast_mut::<State>().last_activity = Some(expired);
        let _ = h.event(
            Event::Window(window::Event::RedrawRequested(Instant::now())),
            outside,
        );
        assert!(h.area.activity.get().is_none());
        assert!(h.tree.state.downcast_ref::<State>().last_activity.is_none());

        h.area = h.area.id("test-scroll");
        let mut jump = iced::advanced::widget::operation::scrollable::scroll_to(
            "test-scroll".into(),
            scrollable::AbsoluteOffset {
                x: None,
                y: Some(120.0),
            },
        );
        h.area
            .operate(&mut h.tree, Layout::new(&h.node), &(), &mut jump);
        let (messages, _) = h.event(
            Event::Window(window::Event::RedrawRequested(Instant::now())),
            outside,
        );
        assert_eq!(messages.last().unwrap().absolute_offset().y, 120.0);
        assert!(h.area.activity.get().is_some());
    }
}
