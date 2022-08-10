use axum::{routing::get, Router};
use interceptor::Interceptor;

mod interceptor;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

    // Create an interceptor, and turn it into a *service* that gets added to the root router.
    //
    // This WORKS.
    let interceptor_service =
        Interceptor::with_name("service").intercept("/intercept-service", intercept_service);
    let nest_service =
        interceptor_service.into_service(axum::routing::any(|| async { "fallback" }));

    // Create an interceptor, and add it via `.layer` to a Router. At the root level, this works
    // fine, but when added via a Router, it doesn't work.
    //
    // THIS IS WHAT DOES NOT WORK.
    let nested_interceptor =
        Interceptor::with_name("nested").intercept("/intercept-nest", intercept_nest);
    let nest_router = Router::new()
        .route("/regular", get(regular))
        .layer(nested_interceptor);

    // Create an interceptor, and add it via `.layer` to the root Router.
    //
    // This WORKS.
    let root_interceptor =
        Interceptor::with_name("root").intercept("/intercept-root", intercept_root);

    let root_router = Router::new()
        .nest("/nest-service", nest_service) // works
        .nest("/nest-router", nest_router) // NEVER CALLS THE INTERCEPTOR
        .route("/regular", get(regular))
        .layer(root_interceptor); // works

    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(root_router.into_make_service())
        .await
        .unwrap();
}

async fn regular() -> &'static str {
    "regular"
}

async fn intercept_nest() -> &'static str {
    "intercept_nest"
}

async fn intercept_service() -> &'static str {
    "intercept_service"
}

async fn intercept_root() -> &'static str {
    "intercept_root"
}

#[cfg(test)]
mod tests {
    async fn get(url: &str) -> String {
        reqwest::get(url).await.unwrap().text().await.unwrap()
    }

    #[tokio::test]
    async fn test_root_interceptor() {
        let response = get("http://localhost:3000/intercept-root").await;
        assert_eq!(response, "intercept_root");
    }

    #[tokio::test]
    async fn test_nested_service_interceptor() {
        let response = get("http://localhost:3000/nest-service/intercept-service").await;
        assert_eq!(response, "intercept_service");
    }

    #[tokio::test]
    async fn test_nested_router_interceptor() {
        // THIS FAILS BECAUSE THE INTERCEPTOR NEVER RUNS

        let response = get("http://localhost:3000/nest-router/intercept-nest").await;
        assert_eq!(response, "intercept_nest");
    }

    #[tokio::test]
    async fn test_nested_router_regular() {
        let response = get("http://localhost:3000/nest-router/regular").await;
        assert_eq!(response, "regular");
    }

    #[tokio::test]
    async fn test_root_regular() {
        let response = get("http://localhost:3000/regular").await;
        assert_eq!(response, "regular");
    }
}
