use super::TableOutput;

pub fn render<T: TableOutput>(value: &T) {
    value.render_table();
}
