use std::cmp::min;

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};
use tui_input::Input;

pub(crate) enum Dialog {
    ConfirmCancelJob(String),
    SelectCancelSignal { id: String, selected_signal: usize },
    EditTimeLimit { id: String, input: Input },
    CommandError { command: String, output: String },
}

pub(crate) const SCANCEL_SIGNALS: &[&str] =
    &["TERM", "INT", "HUP", "USR1", "USR2", "STOP", "CONT", "KILL"];
const DIALOG_WIDTH: u16 = 80;

pub(crate) fn signal_index_for_digit(digit: char) -> Option<usize> {
    let value = digit.to_digit(10)? as usize;
    if value == 0 { None } else { Some(value - 1) }
}

pub(crate) fn validated_time_limit(input: &Input) -> Option<String> {
    let time_limit = input.value().trim();
    if time_limit.is_empty() {
        None
    } else {
        Some(time_limit.to_string())
    }
}

pub(crate) fn render_dialog(f: &mut Frame, dialog: &Dialog) {
    fn centered_dialog_area(width: u16, lines: u16, viewport: Rect) -> Rect {
        let dialog_width = min(width, viewport.width);
        let dialog_height = min(lines, viewport.height);
        let dialog_x = viewport.x + viewport.width.saturating_sub(dialog_width) / 2;
        let dialog_y = viewport.y + viewport.height.saturating_sub(dialog_height) / 2;

        Rect::new(dialog_x, dialog_y, dialog_width, dialog_height)
    }

    match dialog {
        Dialog::ConfirmCancelJob(id) => {
            let dialog = Paragraph::new(Line::from(vec![
                Span::raw("Cancel job "),
                Span::styled(id, Style::default().add_modifier(Modifier::BOLD)),
                Span::raw("?"),
            ]))
            .style(Style::default().fg(Color::White))
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .title("─Cancel")
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .style(Style::default().fg(Color::Green)),
            );

            let area = centered_dialog_area(DIALOG_WIDTH, 3, f.area());
            f.render_widget(Clear, area);
            f.render_widget(dialog, area);
        }
        Dialog::SelectCancelSignal {
            id,
            selected_signal,
        } => {
            let mut rows = vec![
                Line::from(vec![
                    Span::raw("Send signal to job "),
                    Span::styled(id, Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(":"),
                ]),
                Line::default(),
            ];
            rows.extend(SCANCEL_SIGNALS.iter().enumerate().map(|(i, signal)| {
                let signal_style = if i == *selected_signal {
                    Style::default().fg(Color::Black).bg(Color::Green)
                } else {
                    Style::default()
                };
                let shortcut_style = signal_style.add_modifier(Modifier::DIM);
                Line::from(vec![
                    Span::styled(format!("{}. ", i + 1), shortcut_style),
                    Span::styled(*signal, signal_style),
                ])
            }));

            let dialog = Paragraph::new(Text::from(rows))
                .style(Style::default().fg(Color::White))
                .wrap(Wrap { trim: true })
                .block(
                    Block::default()
                        .title("─Signal")
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .style(Style::default().fg(Color::Green)),
                );

            let area =
                centered_dialog_area(DIALOG_WIDTH, SCANCEL_SIGNALS.len() as u16 + 4, f.area());
            f.render_widget(Clear, area);
            f.render_widget(dialog, area);
        }
        Dialog::EditTimeLimit { id, input } => {
            let block = Block::default()
                .title("─Time Limit")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .style(Style::default().fg(Color::Green));

            let area = centered_dialog_area(DIALOG_WIDTH, 3, f.area());
            let inner = block.inner(area);
            let prompt_prefix = "Set time limit for job ";
            let prompt_suffix = ": ";
            let prompt_width = (prompt_prefix.chars().count()
                + id.chars().count()
                + prompt_suffix.chars().count()) as u16;
            let available_width = inner.width.saturating_sub(prompt_width).max(1) as usize;
            let scroll = input.visual_scroll(available_width);
            let visible_value = input
                .value()
                .chars()
                .skip(scroll)
                .take(available_width)
                .collect::<String>();
            let dialog = Paragraph::new(Line::from(vec![
                Span::raw(prompt_prefix),
                Span::styled(id, Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(prompt_suffix),
                Span::styled(visible_value, Style::default().fg(Color::Blue)),
            ]))
            .style(Style::default().fg(Color::White))
            .block(block);

            f.render_widget(Clear, area);
            f.render_widget(dialog, area);

            let cursor_offset = input.visual_cursor().saturating_sub(scroll) as u16;
            let cursor_x = inner
                .x
                .saturating_add(prompt_width)
                .saturating_add(cursor_offset)
                .min(inner.x.saturating_add(inner.width.saturating_sub(1)));
            let cursor_y = inner.y;
            f.set_cursor_position((cursor_x, cursor_y));
        }
        Dialog::CommandError { command, output } => {
            let dialog_text = format!("Command: {command}\n\n{output}");
            let lines = dialog_text
                .lines()
                .count()
                .saturating_add(2)
                .min(u16::MAX as usize) as u16;
            let dialog = Paragraph::new(dialog_text)
                .style(Style::default().fg(Color::White))
                .wrap(Wrap { trim: false })
                .block(
                    Block::default()
                        .title("─Command Error")
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .style(Style::default().fg(Color::Red)),
                );

            let area = centered_dialog_area(DIALOG_WIDTH, lines, f.area());
            f.render_widget(Clear, area);
            f.render_widget(dialog, area);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validated_time_limit() {
        assert_eq!(validated_time_limit(&Input::new("".to_string())), None);
        assert_eq!(validated_time_limit(&Input::new("   ".to_string())), None);
        assert_eq!(
            validated_time_limit(&Input::new(" 01:00:00 ".to_string())),
            Some("01:00:00".to_string())
        );
    }
}
