//
// Web Headers and caching
//
use rocket::fairing::{Fairing, Info, Kind};
use rocket::http::{ContentType, Header, HeaderMap, Method, Status};
use rocket::response::{self, Responder};
use rocket::{Data, Request, Response, Rocket};
use std::io::Cursor;

pub struct AppHeaders();

impl Fairing for AppHeaders {
    fn info(&self) -> Info {
        Info {
            name: "Application Headers",
            kind: Kind::Response,
        }
    }

    fn on_response<'a>(
        &'a self,
        _req: &'a Request<'_>,
        res: &'a mut Response<'_>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            res.set_raw_header("Feature-Policy", "accelerometer 'none'; ambient-light-sensor 'none'; autoplay 'none'; camera 'none'; encrypted-media 'none'; fullscreen 'none'; geolocation 'none'; gyroscope 'none'; magnetometer 'none'; microphone 'none'; midi 'none'; payment 'none'; picture-in-picture 'none'; sync-xhr 'self' https://haveibeenpwned.com https://twofactorauth.org; usb 'none'; vr 'none'");
            res.set_raw_header("Referrer-Policy", "same-origin");
            res.set_raw_header("X-Frame-Options", "SAMEORIGIN");
            res.set_raw_header("X-Content-Type-Options", "nosniff");
            res.set_raw_header("X-XSS-Protection", "1; mode=block");
            let csp = "frame-ancestors 'self' chrome-extension://nngceckbapebfimnlniiiahkandclblb moz-extension://*;";
            res.set_raw_header("Content-Security-Policy", csp);

            // Disable cache unless otherwise specified
            if !res.headers().contains("cache-control") {
                res.set_raw_header("Cache-Control", "no-cache, no-store, max-age=0");
            }
        })
    }
}

pub struct CORS();

impl CORS {
    fn get_header(headers: &HeaderMap, name: &str) -> String {
        match headers.get_one(name) {
            Some(h) => h.to_string(),
            _ => "".to_string(),
        }
    }

    fn valid_url(url: String) -> String {
        match url.as_ref() {
            "file://" => "*".to_string(),
            _ => url,
        }
    }
}

impl Fairing for CORS {
    fn info(&self) -> Info {
        Info {
            name: "CORS",
            kind: Kind::Response,
        }
    }

    fn on_response<'a>(
        &'a self,
        request: &'a Request<'_>,
        response: &'a mut Response<'_>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            let req_headers = request.headers();

            // We need to explicitly get the Origin header for Access-Control-Allow-Origin
            let req_allow_origin = CORS::valid_url(CORS::get_header(&req_headers, "Origin"));

            response.set_header(Header::new("Access-Control-Allow-Origin", req_allow_origin));

            if request.method() == Method::Options {
                let req_allow_headers = CORS::get_header(&req_headers, "Access-Control-Request-Headers");
                let req_allow_method = CORS::get_header(&req_headers, "Access-Control-Request-Method");

                response.set_header(Header::new("Access-Control-Allow-Methods", req_allow_method));
                response.set_header(Header::new("Access-Control-Allow-Headers", req_allow_headers));
                response.set_header(Header::new("Access-Control-Allow-Credentials", "true"));
                response.set_status(Status::Ok);
                response.set_header(ContentType::Plain);
                response.set_sized_body(Cursor::new(""));
            }
        })
    }
}

pub struct Cached<R>(R, &'static str);

impl<R> Cached<R> {
    pub fn long(r: R) -> Cached<R> {
        // 7 days
        Cached(r, "public, max-age=604800")
    }

    pub fn short(r: R) -> Cached<R> {
        // 10 minutes
        Cached(r, "public, max-age=600")
    }
}

impl<'r, R: 'r + Responder<'r> + Send> Responder<'r> for Cached<R> {
    fn respond_to(self, req: &'r Request<'_>) -> response::ResultFuture<'r> {
        use futures::future::FutureExt;
        let cache_value = self.1;

        self.0
            .respond_to(req)
            .then(move |res| async move {
                res.and_then(|mut r| {
                    r.set_raw_header("Cache-Control", cache_value);
                    Ok(r)
                })
            })
            .boxed()
    }
}

// Log all the routes from the main base paths list, and the attachments endoint
// Effectively ignores, any static file route, and the alive endpoint
const LOGGED_ROUTES: [&str; 6] = [
    "/api",
    "/admin",
    "/identity",
    "/icons",
    "/notifications/hub/negotiate",
    "/attachments",
];

// Boolean is extra debug, when true, we ignore the whitelist above and also print the mounts
pub struct BetterLogging(pub bool);
impl Fairing for BetterLogging {
    fn info(&self) -> Info {
        Info {
            name: "Better Logging",
            kind: Kind::Launch | Kind::Request | Kind::Response,
        }
    }

    fn on_launch(&self, rocket: &Rocket) {
        if self.0 {
            info!(target: "routes", "Routes loaded:");
            for route in rocket.routes() {
                if route.rank < 0 {
                    info!(target: "routes", "{:<6} {}", route.method, route.uri);
                } else {
                    info!(target: "routes", "{:<6} {} [{}]", route.method, route.uri, route.rank);
                }
            }
        }

        let config = rocket.config();
        let scheme = if config.tls_enabled() { "https" } else { "http" };
        let addr = format!("{}://{}:{}", &scheme, &config.address, &config.port);
        info!(target: "start", "Rocket has launched from {}", addr);
    }

    fn on_request<'a>(
        &'a self,
        request: &'a mut Request<'_>,
        _: &'a Data,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            let method = request.method();
            if !self.0 && method == Method::Options {
                return;
            }
            let uri = request.uri();
            let uri_path = uri.path();
            if self.0 || LOGGED_ROUTES.iter().any(|r| uri_path.starts_with(r)) {
                match uri.query() {
                    Some(q) => info!(target: "request", "{} {}?{}", method, uri_path, &q[..q.len().min(30)]),
                    None => info!(target: "request", "{} {}", method, uri_path),
                };
            }
        })
    }

    fn on_response<'a>(
        &'a self,
        request: &'a Request<'_>,
        response: &'a mut Response<'_>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if !self.0 && request.method() == Method::Options {
                return;
            }
            let uri_path = request.uri().path();
            if self.0 || LOGGED_ROUTES.iter().any(|r| uri_path.starts_with(r)) {
                let status = response.status();
                if let Some(ref route) = request.route() {
                    info!(target: "response", "{} => {} {}", route, status.code, status.reason)
                } else {
                    info!(target: "response", "{} {}", status.code, status.reason)
                }
            }
        })
    }
}

//
// File handling
//
use std::fs::{self, File};
use std::io::{Read, Result as IOResult};
use std::path::Path;

pub fn file_exists(path: &str) -> bool {
    Path::new(path).exists()
}

pub fn read_file(path: &str) -> IOResult<Vec<u8>> {
    let mut contents: Vec<u8> = Vec::new();

    let mut file = File::open(Path::new(path))?;
    file.read_to_end(&mut contents)?;

    Ok(contents)
}

pub fn read_file_string(path: &str) -> IOResult<String> {
    let mut contents = String::new();

    let mut file = File::open(Path::new(path))?;
    file.read_to_string(&mut contents)?;

    Ok(contents)
}

pub fn delete_file(path: &str) -> IOResult<()> {
    let res = fs::remove_file(path);

    if let Some(parent) = Path::new(path).parent() {
        // If the directory isn't empty, this returns an error, which we ignore
        // We only want to delete the folder if it's empty
        fs::remove_dir(parent).ok();
    }

    res
}

pub struct LimitedReader<'a> {
    reader: &'a mut dyn std::io::Read,
    limit: usize, // In bytes
    count: usize,
}
impl<'a> LimitedReader<'a> {
    pub fn new(reader: &'a mut dyn std::io::Read, limit: usize) -> LimitedReader<'a> {
        LimitedReader {
            reader,
            limit,
            count: 0,
        }
    }
}

impl<'a> std::io::Read for LimitedReader<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.count += buf.len();

        if self.count > self.limit {
            Ok(0) // End of the read
        } else {
            self.reader.read(buf)
        }
    }
}

const UNITS: [&str; 6] = ["bytes", "KB", "MB", "GB", "TB", "PB"];

pub fn get_display_size(size: i32) -> String {
    let mut size: f64 = size.into();
    let mut unit_counter = 0;

    loop {
        if size > 1024. {
            size /= 1024.;
            unit_counter += 1;
        } else {
            break;
        }
    }

    // Round to two decimals
    size = (size * 100.).round() / 100.;
    format!("{} {}", size, UNITS[unit_counter])
}

pub fn get_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

//
// String util methods
//

use std::ops::Try;
use std::str::FromStr;

pub fn upcase_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

pub fn try_parse_string<S, T, U>(string: impl Try<Ok = S, Error = U>) -> Option<T>
where
    S: AsRef<str>,
    T: FromStr,
{
    if let Ok(Ok(value)) = string.into_result().map(|s| s.as_ref().parse::<T>()) {
        Some(value)
    } else {
        None
    }
}

//
// Env methods
//

use std::env;

pub fn get_env<V>(key: &str) -> Option<V>
where
    V: FromStr,
{
    try_parse_string(env::var(key))
}

//
// Date util methods
//

use chrono::NaiveDateTime;

const DATETIME_FORMAT: &str = "%Y-%m-%dT%H:%M:%S%.6fZ";

pub fn format_date(date: &NaiveDateTime) -> String {
    date.format(DATETIME_FORMAT).to_string()
}

//
// Deserialization methods
//

use std::fmt;

use serde::de::{self, DeserializeOwned, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{self, Value};

pub type JsonMap = serde_json::Map<String, Value>;

#[derive(PartialEq, Serialize, Deserialize)]
pub struct UpCase<T: DeserializeOwned> {
    #[serde(deserialize_with = "upcase_deserialize")]
    #[serde(flatten)]
    pub data: T,
}

// https://github.com/serde-rs/serde/issues/586
pub fn upcase_deserialize<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: DeserializeOwned,
    D: Deserializer<'de>,
{
    let d = deserializer.deserialize_any(UpCaseVisitor)?;
    T::deserialize(d).map_err(de::Error::custom)
}

struct UpCaseVisitor;

impl<'de> Visitor<'de> for UpCaseVisitor {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an object or an array")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut result_map = JsonMap::new();

        while let Some((key, value)) = map.next_entry()? {
            result_map.insert(upcase_first(key), upcase_value(value));
        }

        Ok(Value::Object(result_map))
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut result_seq = Vec::<Value>::new();

        while let Some(value) = seq.next_element()? {
            result_seq.push(upcase_value(value));
        }

        Ok(Value::Array(result_seq))
    }
}

fn upcase_value(value: Value) -> Value {
    if let Value::Object(map) = value {
        let mut new_value = json!({});

        for (key, val) in map.into_iter() {
            let processed_key = _process_key(&key);
            new_value[processed_key] = upcase_value(val);
        }
        new_value
    } else if let Value::Array(array) = value {
        // Initialize array with null values
        let mut new_value = json!(vec![Value::Null; array.len()]);

        for (index, val) in array.into_iter().enumerate() {
            new_value[index] = upcase_value(val);
        }
        new_value
    } else {
        value
    }
}

fn _process_key(key: &str) -> String {
    match key.to_lowercase().as_ref() {
        "ssn" => "SSN".into(),
        _ => self::upcase_first(key),
    }
}

//
// Retry methods
//

pub fn retry<F, T, E>(func: F, max_tries: i32) -> Result<T, E>
where
    F: Fn() -> Result<T, E>,
{
    use std::{thread::sleep, time::Duration};
    let mut tries = 0;

    loop {
        match func() {
            ok @ Ok(_) => return ok,
            err @ Err(_) => {
                tries += 1;

                if tries >= max_tries {
                    return err;
                }

                sleep(Duration::from_millis(500));
            }
        }
    }
}
