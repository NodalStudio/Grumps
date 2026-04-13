use worker::*;
pub fn handle(_: Request, _: RouteContext<()>) -> Result<Response> { Response::ok("ok") }
