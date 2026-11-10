use crate::{
    AppError, WindowOp,
    adapter::PlatformAdapter,
    commands::window_target::{AppArgs, window_op_with_request},
};
use serde_json::{Value, json};

pub struct MoveWindowArgs {
    pub app: Option<String>,
    pub window_id: Option<String>,
    pub x: f64,
    pub y: f64,
}

pub fn execute(args: MoveWindowArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    crate::Point {
        x: args.x,
        y: args.y,
    }
    .validate()?;
    window_op_with_request(
        AppArgs {
            app: args.app,
            window_id: args.window_id,
        },
        adapter,
        WindowOp::Move {
            x: args.x,
            y: args.y,
        },
        json!({ "x": args.x, "y": args.y }),
    )
}
