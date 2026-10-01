//! Structured replies: tables, boards, charts, and field cards.

use super::shell;
use crate::theme::ink;
use gpui::prelude::*;
use gpui::{div, px, AnyElement, Context, SharedString};
use serde_json::Value;

fn cell(v: &Value) -> String {
  match v {
    Value::String(s) => s.clone(),
    Value::Null => String::new(),
    v => v.to_string(),
  }
}

pub fn render<T: 'static>(kind: &str, title: &str, data: &Value, cx: &mut Context<T>) -> AnyElement {
  let ink = ink(cx);
  let mut card = shell(cx).child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(SharedString::from(title.to_string())));
  match kind {
    "table" => {
      let cols: Vec<String> = data["columns"].as_array().map(|a| a.iter().map(cell).collect()).unwrap_or_default();
      let mut table = guise::Table::new().head(cols).striped(true).highlight_on_hover(true);
      for r in data["rows"].as_array().into_iter().flatten() {
        let row: Vec<String> = match r {
          Value::Array(a) => a.iter().map(cell).collect(),
          Value::Object(o) => o.values().map(cell).collect(),
          v => vec![cell(v)],
        };
        table = table.row(row);
      }
      card = card.child(div().id("table").overflow_x_scroll().child(table));
    }
    "board" => {
      let mut row = div().flex().gap(px(10.0)).overflow_hidden();
      for col in data["columns"].as_array().into_iter().flatten() {
        let mut c = div()
          .w(px(180.0))
          .flex_none()
          .flex()
          .flex_col()
          .gap(px(6.0))
          .p(px(8.0))
          .rounded(px(8.0))
          .bg(ink.hover)
          .child(div().text_size(px(12.0)).font_weight(gpui::FontWeight::SEMIBOLD).child(cell(&col["title"])));
        for item in col["items"].as_array().into_iter().flatten() {
          let text = if item.is_object() { cell(&item["title"]) } else { cell(item) };
          c = c.child(div().p(px(8.0)).rounded(px(6.0)).bg(ink.body).text_size(px(12.5)).child(text));
        }
        row = row.child(c);
      }
      card = card.child(row);
    }
    "chart" => {
      let labels: Vec<String> = data["labels"].as_array().map(|a| a.iter().map(cell).collect()).unwrap_or_default();
      let series: Vec<(String, Vec<f32>)> = data["series"]
        .as_array()
        .map(|a| {
          a.iter()
            .map(|s| (cell(&s["name"]), s["values"].as_array().map(|v| v.iter().filter_map(|x| x.as_f64()).map(|x| x as f32).collect()).unwrap_or_default()))
            .collect()
        })
        .unwrap_or_default();
      let first = series.first().map(|s| s.1.clone()).unwrap_or_default();
      let chart: AnyElement = match data["type"].as_str().unwrap_or("bar") {
        "line" => {
          let mut c = guise::LineChart::new(first).labels(labels.clone()).axis().hover().width(560.0).height(220.0);
          for (name, vals) in series.iter().skip(1) {
            c = c.add_series(name.clone(), vals.clone());
          }
          c.into_any_element()
        }
        "pie" => guise::PieChart::entries(labels.iter().cloned().zip(first)).size(200.0).into_any_element(),
        _ => guise::BarChart::entries(labels.iter().cloned().zip(first)).axis().hover().width(560.0).height(220.0).into_any_element(),
      };
      card = card.child(chart);
      if series.len() > 1 {
        card = card.child(div().text_size(px(11.0)).text_color(ink.dimmed).child(series.iter().map(|s| s.0.clone()).collect::<Vec<_>>().join(" · ")));
      }
    }
    _ => {
      let fields = data["fields"].as_object().cloned().unwrap_or_else(|| data.as_object().cloned().unwrap_or_default());
      for (k, v) in fields {
        card = card.child(
          div()
            .flex()
            .gap(px(12.0))
            .text_size(px(13.0))
            .child(div().w(px(140.0)).flex_none().text_color(ink.dimmed).child(k))
            .child(div().flex_1().child(cell(&v))),
        );
      }
    }
  }
  card.into_any_element()
}
