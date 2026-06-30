//! In-window "New Agent" dialog: pick a provider, name it, and either choose a
//! role preset or describe a custom one. On create it queues a `relay launch`
//! command that the workspace turns into a split. Built from guise components
//! (Modal, Select, TextInput, SegmentedControl, Button); hosted by
//! `WorkspaceView` (see `root/render.rs`), opened via
//! `WorkspaceView::open_new_agent`.

use gpui::prelude::*;
use gpui::{div, Context, Entity, KeyDownEvent, Subscription, WeakEntity, Window};

use guise::{
    Button, Group, Justify, Modal, SegmentedControl, SegmentedControlEvent, Select, Size, Stack,
    Text, TextInput, TextInputEvent, Variant,
};

use crate::root::WorkspaceView;

pub struct NewAgentDialog {
    workspace: WeakEntity<WorkspaceView>,
    opts: config::Options,
    providers: Vec<String>,
    roles: Vec<String>,
    provider: Entity<Select>,
    name: Entity<TextInput>,
    kind: Entity<SegmentedControl>,
    role: Entity<Select>,
    desc: Entity<TextInput>,
    /// True when the "Custom" role tab is selected (free-form description).
    custom: bool,
    /// Which text field Tab should focus next (false = name, true = desc).
    on_desc: bool,
    _subs: Vec<Subscription>,
}

impl NewAgentDialog {
    pub fn new(
        workspace: WeakEntity<WorkspaceView>,
        opts: config::Options,
        providers: Vec<String>,
        roles: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let custom = roles.is_empty();
        let provider = cx.new(|cx| {
            Select::new(cx)
                .label("Provider")
                .placeholder("none enabled")
                .data(providers.clone())
        });
        let name = cx.new(|cx| TextInput::new(cx).label("Name").placeholder("agent name"));
        let kind = cx.new(|cx| {
            SegmentedControl::new(cx)
                .data(["Preset", "Custom"])
                .selected(if custom { 1 } else { 0 })
        });
        let role = cx.new(|cx| Select::new(cx).label("Role").data(roles.clone()));
        let desc =
            cx.new(|cx| TextInput::new(cx).label("Describe").placeholder("what this agent does"));

        window.focus(&name.read(cx).focus_handle(), cx);

        let me = cx.entity().downgrade();
        let mut subs = Vec::new();
        subs.push(cx.subscribe(&kind, |this, _src, event: &SegmentedControlEvent, cx| {
            this.custom = event.0 == 1;
            cx.notify();
        }));
        for field in [&name, &desc] {
            let me = me.clone();
            subs.push(window.subscribe(field, cx, move |_src, event, window, app| {
                if let TextInputEvent::Submit(_) = event {
                    me.update(app, |this, cx| this.commit(window, cx)).ok();
                }
            }));
        }

        Self {
            workspace,
            opts,
            providers,
            roles,
            provider,
            name,
            kind,
            role,
            desc,
            custom,
            on_desc: false,
            _subs: subs,
        }
    }

    fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.name.read(cx).text().trim().to_string();
        if name.is_empty() || self.providers.is_empty() {
            self.close(window, cx);
            return;
        }
        let pi = self.provider.read(cx).selected_index().unwrap_or(0);
        let provider = self.providers[pi.min(self.providers.len() - 1)].clone();
        let (role, task) = if self.custom {
            (None, Some(self.desc.read(cx).text().trim().to_string()))
        } else {
            let ri = self.role.read(cx).selected_index().unwrap_or(0);
            (self.roles.get(ri).cloned(), None)
        };
        crate::relay::save_agent_def(crate::relay::AgentDef {
            name: name.clone(),
            provider: provider.clone(),
            role: role.clone(),
            task: task.clone(),
        });
        let cmd = crate::relay::launch_agent_command(
            &self.opts,
            &provider,
            &name,
            role.as_deref(),
            task.as_deref(),
        );
        self.workspace
            .update(cx, |ws, cx| {
                ws.create_agent(&cmd, window, cx);
                ws.close_modal(window, cx);
            })
            .ok();
    }

    fn close(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.workspace
            .update(cx, |ws, cx| ws.close_modal(window, cx))
            .ok();
    }

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let ks = &event.keystroke;
        if ks.key == "escape" || (ks.modifiers.platform && ks.key == "w") {
            self.close(window, cx);
            cx.stop_propagation();
            return;
        }
        // Tab cycles between the two text fields (only meaningful in Custom,
        // where both Name and Describe are present).
        if ks.key == "tab" && self.custom {
            self.on_desc = !self.on_desc;
            let field = if self.on_desc { &self.desc } else { &self.name };
            window.focus(&field.read(cx).focus_handle(), cx);
            cx.notify();
            cx.stop_propagation();
        }
    }
}

impl Render for NewAgentDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let workspace = self.workspace.clone();
        let me = cx.entity().downgrade();

        let type_row = Stack::new()
            .gap(Size::Xs)
            .child(Text::new("Type").size(Size::Sm))
            .child(self.kind.clone());

        let role_row = if self.custom {
            self.desc.clone().into_any_element()
        } else {
            self.role.clone().into_any_element()
        };

        let footer = Group::new()
            .justify(Justify::End)
            .gap(Size::Sm)
            .child(
                Button::new("agent-cancel", "Cancel")
                    .variant(Variant::Default)
                    .on_click({
                        let workspace = workspace.clone();
                        move |_ev, window, app| {
                            workspace
                                .update(app, |ws, cx| ws.close_modal(window, cx))
                                .ok();
                        }
                    }),
            )
            .child(
                Button::new("agent-create", "Create")
                    .variant(Variant::Filled)
                    .on_click(move |_ev, window, app| {
                        me.update(app, |this, cx| this.commit(window, cx)).ok();
                    }),
            );

        div()
            .on_key_down(cx.listener(Self::on_key))
            .child(
                Modal::new()
                    .title("New Agent")
                    .width(460.0)
                    .on_close(move |_ev, window, app| {
                        workspace
                            .update(app, |ws, cx| ws.close_modal(window, cx))
                            .ok();
                    })
                    .child(
                        Text::new("Run an AI agent in a new split of this workspace.")
                            .dimmed()
                            .size(Size::Sm),
                    )
                    .child(self.provider.clone())
                    .child(self.name.clone())
                    .child(type_row)
                    .child(role_row)
                    .child(footer),
            )
    }
}
