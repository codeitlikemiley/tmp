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
use tmp_core::schema::{DataSource, Schema, TokenType};

#[derive(PartialEq, Eq, Clone, Copy)]
enum Focus {
    Commands,
    Tokens,
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
    let mut selected_tok_idx = 0;
    let mut focus = Focus::Commands;
    let mut edit_mode = EditMode::Normal;
    let mut test_result: Option<Result<Vec<String>, String>> = None;

    let saved = loop {
        terminal.draw(|f| {
            draw_ui(
                f,
                schema,
                selected_cmd_idx,
                selected_tok_idx,
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
                                    Focus::Commands => Focus::Tokens,
                                    Focus::Tokens => Focus::Commands,
                                };
                            }
                            KeyCode::Up => match focus {
                                Focus::Commands => {
                                    if selected_cmd_idx > 0 {
                                        selected_cmd_idx -= 1;
                                        selected_tok_idx = 0;
                                        test_result = None;
                                    }
                                }
                                Focus::Tokens => {
                                    if selected_tok_idx > 0 {
                                        selected_tok_idx -= 1;
                                        test_result = None;
                                    }
                                }
                            },
                            KeyCode::Down => match focus {
                                Focus::Commands => {
                                    if !schema.commands.is_empty()
                                        && selected_cmd_idx + 1 < schema.commands.len()
                                    {
                                        selected_cmd_idx += 1;
                                        selected_tok_idx = 0;
                                        test_result = None;
                                    }
                                }
                                Focus::Tokens => {
                                    if let Some(cmd) = schema.commands.get(selected_cmd_idx) {
                                        if selected_tok_idx + 1 < cmd.tokens.len() {
                                            selected_tok_idx += 1;
                                            test_result = None;
                                        }
                                    }
                                }
                            },
                            KeyCode::Char('v') => {
                                if let Some(cmd) = schema.commands.get_mut(selected_cmd_idx) {
                                    cmd.verified = !cmd.verified;
                                }
                            }
                            KeyCode::Char('V') => {
                                schema.meta.verified = !schema.meta.verified;
                            }
                            KeyCode::Char('t') => {
                                if let Some(cmd) = schema.commands.get(selected_cmd_idx) {
                                    if let Some(token) = cmd.tokens.get(selected_tok_idx) {
                                        if let Some(ref ds) = token.data_source {
                                            let res = DataResolver::resolve(ds, context);
                                            test_result = Some(res);
                                        } else {
                                            test_result =
                                                Some(Err("No data source configured".to_string()));
                                        }
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
                                .commands
                                .get(selected_cmd_idx)
                                .and_then(|c| c.tokens.get(selected_tok_idx))
                                .and_then(|t| t.data_source.as_ref())
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
                                .commands
                                .get(selected_cmd_idx)
                                .and_then(|c| c.tokens.get(selected_tok_idx))
                                .and_then(|t| t.data_source.as_ref())
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
                                .commands
                                .get(selected_cmd_idx)
                                .and_then(|c| c.tokens.get(selected_tok_idx))
                                .and_then(|t| t.data_source.as_ref())
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
                            if let Some(cmd) = schema.commands.get_mut(selected_cmd_idx) {
                                if let Some(token) = cmd.tokens.get_mut(selected_tok_idx) {
                                    let mut ds =
                                        token.data_source.clone().unwrap_or_else(|| DataSource {
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
                                        token.data_source = None;
                                    } else {
                                        token.data_source = Some(ds);
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
    selected_tok_idx: usize,
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
            " Commands (Meta Schema Verified: {}) ",
            schema.meta.verified
        ))
        .borders(Borders::ALL)
        .border_style(if focus == Focus::Commands {
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

    // Command Info block
    let cmd_info_block = Block::default()
        .title(" Selected Command Info ")
        .borders(Borders::ALL);

    // Tokens List block
    let tokens_block = Block::default()
        .title(" Tokens ")
        .borders(Borders::ALL)
        .border_style(if focus == Focus::Tokens {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::White)
        });

    // Token Details block
    let token_details_block = Block::default()
        .title(" Selected Token Details ")
        .borders(Borders::ALL);

    // RENDER LEFT COMMANDS LIST
    let commands_items: Vec<ListItem> = schema
        .commands
        .iter()
        .enumerate()
        .map(|(i, cmd)| {
            let prefix = if cmd.verified { "[✔] " } else { "[ ] " };
            let style = if i == selected_cmd_idx && focus == Focus::Commands {
                Style::default().bg(Color::Blue).fg(Color::White)
            } else if i == selected_cmd_idx {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };
            ListItem::new(format!("{}{}", prefix, cmd.command)).style(style)
        })
        .collect();

    let commands_list = List::new(commands_items).block(left_block);
    f.render_widget(commands_list, main_chunks[0]);

    // RENDER RIGHT PANE
    if let Some(cmd) = schema.commands.get(selected_cmd_idx) {
        // Render Command Info
        let cmd_info_text = format!(
            "Command: {}\nGroup: {}\nVerified: {}\nDescription: {}",
            cmd.command, cmd.group, cmd.verified, cmd.description
        );
        let cmd_info_paragraph = Paragraph::new(cmd_info_text).block(cmd_info_block);
        f.render_widget(cmd_info_paragraph, right_chunks[0]);

        // Render Tokens List
        let tokens_items: Vec<ListItem> = cmd
            .tokens
            .iter()
            .enumerate()
            .map(|(i, tok)| {
                let req_star = if tok.required { "*" } else { "" };
                let type_str = match tok.token_type {
                    TokenType::String => "String",
                    TokenType::Boolean => "Boolean",
                    TokenType::Enum => "Enum",
                    TokenType::File => "File",
                    TokenType::Number => "Number",
                };
                let style = if i == selected_tok_idx && focus == Focus::Tokens {
                    Style::default().bg(Color::Blue).fg(Color::White)
                } else if i == selected_tok_idx {
                    Style::default().bg(Color::DarkGray)
                } else {
                    Style::default()
                };
                ListItem::new(format!("{}{}: {}", tok.name, req_star, type_str)).style(style)
            })
            .collect();
        let tokens_list = List::new(tokens_items).block(tokens_block);
        f.render_widget(tokens_list, right_chunks[1]);

        // Render Selected Token Details
        if let Some(tok) = cmd.tokens.get(selected_tok_idx) {
            let mut ds_str = "None".to_string();
            if let Some(ref ds) = tok.data_source {
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

            let default_str = tok.default.as_deref().unwrap_or("None");
            let flag_str = tok.flag.as_deref().unwrap_or("None");
            let values_str = tok
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
                tok.name, tok.description, default_str, flag_str, values_str, ds_str, test_str
            );
            let tok_details_paragraph = Paragraph::new(tok_details_text).block(token_details_block);
            f.render_widget(tok_details_paragraph, right_chunks[2]);
        } else {
            let empty_p =
                Paragraph::new("No tokens found for this command.").block(token_details_block);
            f.render_widget(empty_p, right_chunks[2]);
        }
    } else {
        // Render placeholders
        let empty_info = Paragraph::new("No command selected.").block(cmd_info_block);
        let empty_tokens = Paragraph::new("No tokens selected.").block(tokens_block);
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
                .title(" Edit Token Data Source ")
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
