use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame, Terminal,
};
use std::io::{stdout, IsTerminal};
use tmp_core::context::Context;
use tmp_core::resolver::DataResolver;
use tmp_core::schema::{DataSource, ParameterType, Schema};
use tmp_core::traits::Resolver;

#[derive(PartialEq, Eq, Clone, Copy)]
enum Focus {
    Operations,
    Parameters,
}

#[derive(Clone)]
enum EditMode {
    Normal,
    PromptField,
    Editing { field: EditField, input: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditField {
    Command,
    Resolver,
    Parse,
}

pub fn run(schema: &mut Schema, context: &Context) -> Result<bool, Box<dyn std::error::Error>> {
    // Check if terminal is interactive
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        println!("Non-interactive shell detected, bypassing verification TUI.");
        return Ok(false);
    }

    // Enable raw mode and alternate screen
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut selected_cmd_idx = 0;
    let mut selected_param_idx = 0;
    let mut focus = Focus::Operations;
    let mut edit_mode = EditMode::Normal;
    let mut test_result: Option<Result<Vec<String>, String>> = None;

    let saved = loop {
        terminal.draw(|f| {
            draw_ui(
                f,
                schema,
                selected_cmd_idx,
                selected_param_idx,
                focus,
                &edit_mode,
                &test_result,
            );
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == event::KeyEventKind::Release {
                    continue; // Skip key release events on Windows/some terminals
                }

                match &mut edit_mode {
                    EditMode::Normal => {
                        match key.code {
                            KeyCode::Char('q') => {
                                break false;
                            }
                            KeyCode::Esc => {
                                break false;
                            }
                            KeyCode::Char('s') => {
                                // Save and exit
                                if let Err(e) = schema.validate() {
                                    test_result = Some(Err(format!("Validation error: {}", e)));
                                } else {
                                    // Save the schema
                                    if let Err(e) = tmp_core::versioning::save_schema(schema) {
                                        test_result = Some(Err(format!("Failed to save: {}", e)));
                                    } else {
                                        break true;
                                    }
                                }
                            }
                            KeyCode::Tab | KeyCode::Left | KeyCode::Right => {
                                focus = match focus {
                                    Focus::Operations => Focus::Parameters,
                                    Focus::Parameters => Focus::Operations,
                                };
                            }
                            KeyCode::Up => match focus {
                                Focus::Operations => {
                                    if selected_cmd_idx > 0 {
                                        selected_cmd_idx -= 1;
                                        selected_param_idx = 0;
                                        test_result = None;
                                    }
                                }
                                Focus::Parameters => {
                                    if selected_param_idx > 0 {
                                        selected_param_idx -= 1;
                                        test_result = None;
                                    }
                                }
                            },
                            KeyCode::Down => match focus {
                                Focus::Operations => {
                                    if !schema.operations.is_empty()
                                        && selected_cmd_idx + 1 < schema.operations.len()
                                    {
                                        selected_cmd_idx += 1;
                                        selected_param_idx = 0;
                                        test_result = None;
                                    }
                                }
                                Focus::Parameters => {
                                    if let Some(op) = schema.operations.get(selected_cmd_idx) {
                                        if selected_param_idx + 1 < op.parameters.len() {
                                            selected_param_idx += 1;
                                            test_result = None;
                                        }
                                    }
                                }
                            },
                            KeyCode::Char('v') => {
                                if let Some(op) = schema.operations.get_mut(selected_cmd_idx) {
                                    op.verified = !op.verified;
                                }
                            }
                            KeyCode::Char('V') => {
                                schema.meta.verified = !schema.meta.verified;
                            }
                            KeyCode::Char('t') => {
                                if let Some(op) = schema.operations.get(selected_cmd_idx) {
                                    if let Some(param) = op.parameters.get(selected_param_idx) {
                                        let resolver = DataResolver;
                                        let res = resolver.values(param, context);
                                        test_result = Some(res);
                                    }
                                }
                            }
                            KeyCode::Char('e') => {
                                edit_mode = EditMode::PromptField;
                            }
                            _ => {}
                        }
                    }
                    EditMode::PromptField => match key.code {
                        KeyCode::Char('c') => {
                            let current_val = schema
                                .operations
                                .get(selected_cmd_idx)
                                .and_then(|op| op.parameters.get(selected_param_idx))
                                .and_then(|p| p.data_source.as_ref())
                                .and_then(|ds| ds.command.as_ref())
                                .cloned()
                                .unwrap_or_default();
                            edit_mode = EditMode::Editing {
                                field: EditField::Command,
                                input: current_val,
                            };
                        }
                        KeyCode::Char('r') => {
                            let current_val = schema
                                .operations
                                .get(selected_cmd_idx)
                                .and_then(|op| op.parameters.get(selected_param_idx))
                                .and_then(|p| p.data_source.as_ref())
                                .and_then(|ds| ds.resolver.as_ref())
                                .cloned()
                                .unwrap_or_default();
                            edit_mode = EditMode::Editing {
                                field: EditField::Resolver,
                                input: current_val,
                            };
                        }
                        KeyCode::Char('p') => {
                            let current_val = schema
                                .operations
                                .get(selected_cmd_idx)
                                .and_then(|op| op.parameters.get(selected_param_idx))
                                .and_then(|p| p.data_source.as_ref())
                                .map(|ds| ds.parse.clone())
                                .unwrap_or_else(|| "lines".to_string());
                            edit_mode = EditMode::Editing {
                                field: EditField::Parse,
                                input: current_val,
                            };
                        }
                        KeyCode::Esc | KeyCode::Char('q') => {
                            edit_mode = EditMode::Normal;
                        }
                        _ => {}
                    },
                    EditMode::Editing { field, input } => match key.code {
                        KeyCode::Char(c) => {
                            input.push(c);
                        }
                        KeyCode::Backspace => {
                            input.pop();
                        }
                        KeyCode::Esc => {
                            edit_mode = EditMode::Normal;
                        }
                        KeyCode::Enter => {
                            let field = *field;
                            let val = input.clone();
                            if let Some(op) = schema.operations.get_mut(selected_cmd_idx) {
                                if let Some(param) = op.parameters.get_mut(selected_param_idx) {
                                    let mut ds =
                                        param.data_source.clone().unwrap_or_else(|| DataSource {
                                            command: None,
                                            resolver: None,
                                            parse: "lines".to_string(),
                                        });
                                    match field {
                                        EditField::Command => {
                                            ds.command = if val.trim().is_empty() {
                                                None
                                            } else {
                                                Some(val.trim().to_string())
                                            };
                                        }
                                        EditField::Resolver => {
                                            ds.resolver = if val.trim().is_empty() {
                                                None
                                            } else {
                                                Some(val.trim().to_string())
                                            };
                                        }
                                        EditField::Parse => {
                                            ds.parse = if val.trim() == "words" {
                                                "words".to_string()
                                            } else {
                                                "lines".to_string()
                                            };
                                        }
                                    }
                                    if ds.command.is_none() && ds.resolver.is_none() {
                                        param.data_source = None;
                                    } else {
                                        param.data_source = Some(ds);
                                    }
                                }
                            }
                            edit_mode = EditMode::Normal;
                            test_result = None;
                        }
                        _ => {}
                    },
                }
            }
        }
    };

    // Restore terminal
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    Ok(saved)
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn draw_ui(
    f: &mut Frame,
    schema: &Schema,
    selected_cmd_idx: usize,
    selected_param_idx: usize,
    focus: Focus,
    edit_mode: &EditMode,
    test_result: &Option<Result<Vec<String>, String>>,
) {
    let size = f.size();

    // Main split: Left 30%, Right 70%
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(size);

    // Left pane block
    let left_block = Block::default()
        .title(format!(
            " Operations (Meta Schema Verified: {}) ",
            schema.meta.verified
        ))
        .borders(Borders::ALL)
        .border_style(if focus == Focus::Operations {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::White)
        });

    // Right pane split
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(35),
            Constraint::Percentage(40),
        ])
        .split(main_chunks[1]);

    // Operation Info block
    let cmd_info_block = Block::default()
        .title(" Selected Operation Info ")
        .borders(Borders::ALL);

    // Parameters List block
    let tokens_block = Block::default()
        .title(" Parameters ")
        .borders(Borders::ALL)
        .border_style(if focus == Focus::Parameters {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::White)
        });

    // Parameter Details block
    let token_details_block = Block::default()
        .title(" Selected Parameter Details ")
        .borders(Borders::ALL);

    // RENDER LEFT OPERATIONS LIST
    let commands_items: Vec<ListItem> = schema
        .operations
        .iter()
        .enumerate()
        .map(|(i, op)| {
            let prefix = if op.verified { "[✔] " } else { "[ ] " };
            let style = if i == selected_cmd_idx && focus == Focus::Operations {
                Style::default().bg(Color::Blue).fg(Color::White)
            } else if i == selected_cmd_idx {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };
            ListItem::new(format!("{}{}", prefix, op.command)).style(style)
        })
        .collect();

    let commands_list = List::new(commands_items).block(left_block);
    f.render_widget(commands_list, main_chunks[0]);

    // RENDER RIGHT PANE
    if let Some(op) = schema.operations.get(selected_cmd_idx) {
        // Render Operation Info
        let cmd_info_text = format!(
            "Command: {}\nGroup: {}\nVerified: {}\nDescription: {}",
            op.command, op.group, op.verified, op.description
        );
        let cmd_info_paragraph = Paragraph::new(cmd_info_text).block(cmd_info_block);
        f.render_widget(cmd_info_paragraph, right_chunks[0]);

        // Render Parameters List
        let tokens_items: Vec<ListItem> = op
            .parameters
            .iter()
            .enumerate()
            .map(|(i, param)| {
                let req_star = if param.required { "*" } else { "" };
                let type_str = match param.parameter_type {
                    ParameterType::String => "String",
                    ParameterType::Boolean => "Boolean",
                    ParameterType::Enum => "Enum",
                    ParameterType::File => "File",
                    ParameterType::Number => "Number",
                };
                let style = if i == selected_param_idx && focus == Focus::Parameters {
                    Style::default().bg(Color::Blue).fg(Color::White)
                } else if i == selected_param_idx {
                    Style::default().bg(Color::DarkGray)
                } else {
                    Style::default()
                };
                ListItem::new(format!("{}{}: {}", param.name, req_star, type_str)).style(style)
            })
            .collect();
        let tokens_list = List::new(tokens_items).block(tokens_block);
        f.render_widget(tokens_list, right_chunks[1]);

        // Render Selected Parameter Details
        if let Some(param) = op.parameters.get(selected_param_idx) {
            let mut ds_str = "None".to_string();
            if let Some(ref ds) = param.data_source {
                let cmd_part = ds
                    .command
                    .as_ref()
                    .map(|c| format!("Command: {}", c))
                    .unwrap_or_else(|| "Command: None".to_string());
                let res_part = ds
                    .resolver
                    .as_ref()
                    .map(|r| format!("Resolver: {}", r))
                    .unwrap_or_else(|| "Resolver: None".to_string());
                ds_str = format!("{} | {} | Parse: {}", cmd_part, res_part, ds.parse);
            }

            let default_str = param.default.as_deref().unwrap_or("None");
            let flag_str = param.flag.as_deref().unwrap_or("None");
            let values_str = param
                .values
                .as_ref()
                .map(|v| format!("{:?}", v))
                .unwrap_or_else(|| "None".to_string());

            let mut test_str = String::new();
            if let Some(ref res) = test_result {
                match res {
                    Ok(vals) => {
                        test_str = format!("\nLive Test values resolved: {:?}", vals);
                    }
                    Err(e) => {
                        test_str = format!("\nLive Test Error: {}", e);
                    }
                }
            }

            let tok_details_text = format!(
                "Name: {}\nDescription: {}\nDefault: {}\nFlag: {}\nAllowed values: {}\nData Source: {}{}",
                param.name, param.description, default_str, flag_str, values_str, ds_str, test_str
            );
            let tok_details_paragraph = Paragraph::new(tok_details_text).block(token_details_block);
            f.render_widget(tok_details_paragraph, right_chunks[2]);
        } else {
            let empty_p = Paragraph::new("No parameters found for this operation.")
                .block(token_details_block);
            f.render_widget(empty_p, right_chunks[2]);
        }
    } else {
        // Render placeholders
        let empty_info = Paragraph::new("No operation selected.").block(cmd_info_block);
        let empty_tokens = Paragraph::new("No parameters selected.").block(tokens_block);
        let empty_details = Paragraph::new("No details.").block(token_details_block);
        f.render_widget(empty_info, right_chunks[0]);
        f.render_widget(empty_tokens, right_chunks[1]);
        f.render_widget(empty_details, right_chunks[2]);
    }

    // DRAW OVERLAYS FOR EDIT MODE
    match edit_mode {
        EditMode::Normal => {}
        EditMode::PromptField => {
            let block = Block::default()
                .title(" Edit Parameter Data Source ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green));
            let text = "Select data source field to edit:\n\n [c] Command\n [r] Resolver\n [p] Parse mode\n\nPress Esc to cancel";
            let p = Paragraph::new(text)
                .block(block)
                .alignment(Alignment::Center);

            let area = centered_rect(50, 30, size);
            f.render_widget(Clear, area);
            f.render_widget(p, area);
        }
        EditMode::Editing { field, input } => {
            let field_name = match field {
                EditField::Command => "Command",
                EditField::Resolver => "Resolver",
                EditField::Parse => "Parse Mode (lines/words)",
            };
            let block = Block::default()
                .title(format!(" Editing {} ", field_name))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green));
            let text = format!(
                "Current Input:\n\n> {}\n\nPress Enter to save, Esc to cancel",
                input
            );
            let p = Paragraph::new(text).block(block).alignment(Alignment::Left);

            let area = centered_rect(60, 30, size);
            f.render_widget(Clear, area);
            f.render_widget(p, area);
        }
    }
}
