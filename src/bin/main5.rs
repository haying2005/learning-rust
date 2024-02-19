use tracing::{debug, info, span, Level,event};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};

struct User {
    name: &'static str,
    email: &'static str,
}
fn main() {
    tracing_subscriber::registry().with(fmt::layer()).init();

    let user = "ferris";
    let email = "ferris@rust-lang.org";
    event!(Level::TRACE, user, user.email = email);

    // 还可以使用结构体
    let user = User {
        name: "ferris",
        email: "ferris@rust-lang.org",
    };

    // 直接访问结构体字段，无需赋值即可使用
    let span = span!(Level::TRACE, "login", user.name, user.email);
    let _enter = span.enter();

    // 字段名还可以使用字符串
    event!(Level::TRACE, "guid:x-request-id" = "abcdef", "type" = "request", "sdfsdf{} {:?}", user.name, user.email);
}