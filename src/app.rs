use async_trait::async_trait;
use axum::Router as AxumRouter;
use loco_rs::{
    app::{AppContext, Hooks},
    bgworker::Queue,
    boot::{create_app, BootResult, StartMode},
    config::Config,
    controller::AppRoutes,
    environment::Environment,
    task::Tasks,
    Result,
};

use crate::{
    controllers,
    lodestone::{Lodestone, Selectors},
    mcp,
    settings::Settings,
};

pub struct App;

#[async_trait]
impl Hooks for App {
    fn app_name() -> &'static str {
        env!("CARGO_CRATE_NAME")
    }

    fn app_version() -> String {
        format!(
            "{} ({})",
            env!("CARGO_PKG_VERSION"),
            option_env!("BUILD_SHA")
                .or(option_env!("GITHUB_SHA"))
                .unwrap_or("dev")
        )
    }

    async fn boot(
        mode: StartMode,
        environment: &Environment,
        config: Config,
    ) -> Result<BootResult> {
        create_app::<Self>(mode, environment, config).await
    }

    /// Builds the Lodestone client once. Invalid settings or selector files
    /// fail here, at boot, rather than on the first request.
    async fn after_context(ctx: AppContext) -> Result<AppContext> {
        let settings = Settings::from_config(ctx.config.settings.as_ref())?;
        let lodestone = Lodestone::new(settings, Selectors::load()?)?;
        ctx.shared_store.insert(lodestone);
        Ok(ctx)
    }

    fn routes(_ctx: &AppContext) -> AppRoutes {
        AppRoutes::with_default_routes()
            .add_route(controllers::characters::routes())
            .add_route(controllers::free_companies::routes())
            .add_route(controllers::groups::linkshell_routes())
            .add_route(controllers::groups::cwls_routes())
            .add_route(controllers::groups::pvp_team_routes())
    }

    /// The MCP tools call the router built so far, so this must run last.
    async fn after_routes(router: AxumRouter, _ctx: &AppContext) -> Result<AxumRouter> {
        Ok(mcp::mount(router))
    }

    async fn connect_workers(_ctx: &AppContext, _queue: &Queue) -> Result<()> {
        Ok(())
    }

    fn register_tasks(_tasks: &mut Tasks) {}
}
