use crate::{
    AppError, WindowOp,
    adapter::PlatformAdapter,
    commands::window_target::{AppArgs, window_op_with_request},
};
use serde_json::{Value, json};

pub struct ResizeWindowArgs {
    pub app: Option<String>,
    pub window_id: Option<String>,
    pub width: f64,
    pub height: f64,
}

pub fn execute(args: ResizeWindowArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    crate::Rect {
        x: 0.0,
        y: 0.0,
        width: args.width,
        height: args.height,
    }
    .validate()?;
    window_op_with_request(
        AppArgs {
            app: args.app,
            window_id: args.window_id,
        },
        adapter,
        WindowOp::Resize {
            width: args.width,
            height: args.height,
        },
        json!({ "width": args.width, "height": args.height }),
    )
}
