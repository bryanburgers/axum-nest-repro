use std::{collections::HashMap, convert::Infallible};
use tower::{
    util::{BoxCloneService, Oneshot},
    Layer, Service,
};

type Request<B> = http::Request<B>;
type Response = http::Response<axum::body::BoxBody>;

// My actual interceptor is more complicated, used for parsing and dispatching OData-like URLs.
pub struct Interceptor<B> {
    interceptions: HashMap<String, BoxCloneService<Request<B>, Response, Infallible>>,
}

impl<B> Clone for Interceptor<B> {
    fn clone(&self) -> Self {
        Self {
            interceptions: self.interceptions.clone(),
        }
    }
}

impl<S, B> Layer<S> for Interceptor<B>
where
    S: Service<Request<B>, Response = Response, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Service = InterceptorService<B>;

    fn layer(&self, inner: S) -> Self::Service {
        InterceptorService::from_router_and_next(self.clone(), inner)
    }
}

impl<B> Interceptor<B>
where
    B: Send + 'static,
{
    pub fn new() -> Self {
        let interceptions = HashMap::new();

        Self { interceptions }
    }

    pub fn intercept<H, T>(mut self, entity: impl ToString, handler: H) -> Self
    where
        H: axum::handler::Handler<T, B>,
        T: 'static,
    {
        let service = handler.into_service();
        let service = BoxCloneService::new(service);
        self.interceptions.insert(entity.to_string(), service);
        self
    }

    pub fn into_service<S>(self, next: S) -> InterceptorService<B>
    where
        S: Service<Request<B>, Response = Response, Error = Infallible> + Clone + Send + 'static,
        S::Future: Send + 'static,
    {
        InterceptorService::from_router_and_next(self, next)
    }
}

impl<B> Default for Interceptor<B>
where
    B: Send + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

pub struct InterceptorService<B> {
    router: Interceptor<B>,
    next: BoxCloneService<Request<B>, Response, Infallible>,
}

impl<B> Clone for InterceptorService<B> {
    fn clone(&self) -> Self {
        Self {
            router: self.router.clone(),
            next: self.next.clone(),
        }
    }
}

impl<B> InterceptorService<B> {
    pub fn from_router_and_next<S>(router: Interceptor<B>, next: S) -> Self
    where
        S: Service<Request<B>, Response = Response, Error = Infallible> + Clone + Send + 'static,
        S::Future: Send + 'static,
    {
        let service = BoxCloneService::new(next);
        Self {
            router,
            next: service,
        }
    }
}

impl<B> Service<Request<B>> for InterceptorService<B> {
    type Response = Response;
    type Error = Infallible;
    type Future = tower::util::Oneshot<
        tower::util::BoxCloneService<Request<B>, Response, Infallible>,
        Request<B>,
    >;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        if let Some(interception) = self.router.interceptions.get(req.uri().path()) {
            return Oneshot::new(interception.clone(), req);
        }

        Oneshot::new(self.next.clone(), req)
    }
}
