#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum WidgetKind {
    System,
    Network,
    Audio,
    Media,
    Device,
}
