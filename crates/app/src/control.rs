//! Accessible app-level controls matching guise's visual language.

use gpui::prelude::*;
use gpui::{div, px, App, ClickEvent, ElementId, FontWeight, SharedString, Window};
use guise::{ColorName, ColorValue, Size, Variant};

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: SharedString,
    variant: Variant,
    color: ColorValue,
    size: Size,
    disabled: bool,
    on_click: Option<ClickHandler>,
}

impl Button {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            variant: Variant::Filled,
            color: ColorValue::default(),
            size: Size::Sm,
            disabled: false,
            on_click: None,
        }
    }

    pub fn variant(mut self, variant: Variant) -> Self {
        self.variant = variant;
        self
    }

    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }

    fn metrics(&self) -> (f32, f32, f32) {
        match self.size {
            Size::Xs => (30.0, 14.0, 12.0),
            Size::Sm => (36.0, 18.0, 14.0),
            Size::Md => (42.0, 22.0, 16.0),
            Size::Lg => (50.0, 26.0, 18.0),
            Size::Xl => (60.0, 32.0, 20.0),
        }
    }
}

impl RenderOnce for Button {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = guise::theme::theme(cx);
        let surface = guise::style::surface(theme, self.color, self.variant);
        let focus = theme.primary().hsla();
        let (height, padding, font) = self.metrics();
        let radius = theme.radius(theme.default_radius);
        let label = self.label.clone();

        let mut element = div()
            .id(self.id)
            .flex()
            .items_center()
            .justify_center()
            .gap(px(8.0))
            .h(px(height))
            .px(px(padding))
            .rounded(px(radius))
            .bg(surface.bg)
            .text_color(surface.fg)
            .text_size(px(font))
            .font_weight(FontWeight::SEMIBOLD)
            .role(gpui::accesskit::Role::Button)
            .aria_label(label.clone());

        if let Some(border) = surface.border {
            element = element.border_1().border_color(border);
        }
        element = element.child(label);

        if self.disabled {
            element.opacity(0.6)
        } else {
            element = element
                .tab_index(0)
                .cursor_pointer()
                .focus_visible(move |style| style.border_1().border_color(focus))
                .hover(move |style| style.bg(surface.bg_hover));
            if let Some(handler) = self.on_click {
                element = element.on_click(handler);
            }
            element
        }
    }
}

#[derive(IntoElement)]
pub struct Switch {
    id: ElementId,
    checked: bool,
    label: Option<SharedString>,
    aria_label: Option<SharedString>,
    size: Size,
    disabled: bool,
    on_change: Option<ClickHandler>,
}

impl Switch {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            checked: false,
            label: None,
            aria_label: None,
            size: Size::Md,
            disabled: false,
            on_change: None,
        }
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        let label = label.into();
        self.aria_label = Some(label.clone());
        self.label = Some(label);
        self
    }

    pub fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.aria_label = Some(label.into());
        self
    }

    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_change(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    fn height(&self) -> f32 {
        match self.size {
            Size::Xs => 16.0,
            Size::Sm => 20.0,
            Size::Md => 24.0,
            Size::Lg => 30.0,
            Size::Xl => 36.0,
        }
    }
}

impl RenderOnce for Switch {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = guise::theme::theme(cx);
        let height = self.height();
        let width = (height * 1.85).round();
        let knob = height - 4.0;
        let accent = theme.color(ColorName::Blue, theme.primary_shade());
        let checked = self.checked;
        let track_color = if checked {
            accent.hsla()
        } else {
            theme
                .color(ColorName::Gray, if theme.scheme.is_dark() { 6 } else { 4 })
                .hsla()
        };
        let knob_x = if checked { width - knob - 2.0 } else { 2.0 };
        let focus = theme.primary().hsla();
        let label = self
            .aria_label
            .clone()
            .unwrap_or_else(|| SharedString::from("Toggle setting"));

        let track = div()
            .w(px(width))
            .h(px(height))
            .rounded(px(height))
            .bg(track_color)
            .relative()
            .child(
                div()
                    .absolute()
                    .top(px(2.0))
                    .left(px(knob_x))
                    .w(px(knob))
                    .h(px(knob))
                    .rounded(px(knob))
                    .bg(theme.white.hsla()),
            );

        let mut row = div()
            .id(self.id)
            .flex()
            .items_center()
            .gap(px(8.0))
            .role(gpui::accesskit::Role::Switch)
            .aria_label(label)
            .aria_toggled(checked.into())
            .child(track);
        if let Some(label) = self.label {
            row = row.child(
                div()
                    .text_size(px(theme.font_size(self.size)))
                    .text_color(theme.text().hsla())
                    .child(label),
            );
        }

        if self.disabled {
            row.opacity(0.5)
        } else {
            let handler = self.on_change;
            row = row
                .tab_index(0)
                .cursor_pointer()
                .focus_visible(move |style| style.border_1().border_color(focus));
            if handler.is_some() {
                row = row.on_click(move |event, window, cx| {
                    if let Some(handler) = &handler {
                        handler(event, window, cx);
                    }
                });
            }
            row
        }
    }
}
