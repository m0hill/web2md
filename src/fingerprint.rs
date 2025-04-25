use lazy_static::lazy_static;
use rand::{rngs::SmallRng, SeedableRng, Rng, seq::SliceRandom};
use js_sys::Date;
use worker::Headers;

#[derive(Debug, Clone)]
pub struct BrowserVersion {
    pub name: &'static str,
    pub version_prefix: &'static str,
    pub min_version: u32,
    pub max_version: u32,
    pub platform: &'static str,
    pub engine: &'static str,
    pub engine_version: &'static str,
    pub brand_version: String,
}

#[derive(Debug, Clone)]
pub struct BrowserFingerprint {
    pub user_agent: String,
    pub accept_language: String,
    pub platform: String,
    pub viewport: (u32, u32),
    pub color_depth: u32,
    pub pixel_ratio: f32,
    pub timezone_offset: i32,
    pub hardware_concurrency: u8,
    pub memory_gb: u8,
    pub webgl_vendor: String,
    pub webgl_renderer: String,
    pub architecture: &'static str,
    pub bitness: &'static str,
    pub platform_version: String,
    pub browser_version: String,
    pub mobile: bool,
    pub headers: Vec<(String, String)>,
    pub connection_type: &'static str,
    pub preferred_languages: Vec<String>,
}

lazy_static! {
    static ref BROWSER_CONFIGS: Vec<BrowserVersion> = vec![
        BrowserVersion {
            name: "Chrome",
            version_prefix: "Chrome/",
            min_version: 90,
            max_version: 119,
            platform: "Windows",
            engine: "Blink",
            engine_version: "90.0.0.0",
            brand_version: "90.0.6099.109".to_string(),
        },
        BrowserVersion {
            name: "Firefox",
            version_prefix: "Firefox/",
            min_version: 90,
            max_version: 119,
            platform: "Windows",
            engine: "Gecko",
            engine_version: "90.0",
            brand_version: "90.0".to_string(),
        },
        BrowserVersion {
            name: "Safari",
            version_prefix: "Version/",
            min_version: 15,
            max_version: 17,
            platform: "Macintosh",
            engine: "WebKit",
            engine_version: "15.0",
            brand_version: "15.0".to_string(),
        }
    ];

    static ref VIEWPORT_SIZES: [(u32, u32); 5] = [
        (1920, 1080),
        (1366, 768),
        (1536, 864),
        (1440, 900),
        (1280, 720),
    ];

    static ref WEBGL_CONFIGS: Vec<(&'static str, &'static str)> = vec![
        ("ANGLE (Apple, Apple M1 Pro, OpenGL 4.1)", "ANGLE (Metal, Apple M1 Pro, OpenGL 4.1)"),
        ("ANGLE (Intel, Intel(R) UHD Graphics, OpenGL 4.1)", "ANGLE (Metal, Intel(R) UHD Graphics, OpenGL 4.1)"),
        ("ANGLE (AMD, AMD Radeon Pro 5500M, OpenGL 4.1)", "ANGLE (Metal, AMD Radeon Pro 5500M, OpenGL 4.1)"),
    ];

    static ref LANGUAGES: Vec<&'static str> = vec![
        "en-US", "en-GB", "fr-FR", "de-DE", "es-ES", "it-IT", "ja-JP",
    ];

    static ref ARCHITECTURES: Vec<&'static str> = vec![
        "aarch64", "x86_64",
    ];

    static ref PLATFORM_VERSIONS: Vec<&'static str> = vec![
        "10_15_7", "11_0_0", "11_2_3", "11_3_1", "11_4_0", "11_5_2", "11_6_0",
        "12_0_0", "12_1_0", "12_2_1", "12_3_0", "12_4_0", "12_5_0", "12_6_0",
        "13_0_0", "13_1_0", "13_2_0", "13_3_0", "13_4_0", "13_5_0",
    ];

    static ref CONNECTION_TYPES: Vec<&'static str> = vec![
        "wifi", "4g", "3g",
    ];
}

impl BrowserFingerprint {
    pub fn generate() -> Self {
        let now = Date::now() as u64;
        let mut rng = SmallRng::seed_from_u64(now);

        let browser = BROWSER_CONFIGS.choose(&mut rng).unwrap().clone();

        let viewport = VIEWPORT_SIZES.choose(&mut rng).unwrap();

        let color_depth = if rng.gen_bool(0.9) { 24 } else { 32 };

        let pixel_ratio = if rng.gen_bool(0.7) {
            1.0
        } else {
            [1.5, 2.0, 2.25, 3.0].choose(&mut rng).unwrap().clone()
        };

        let timezone_offset = [-480, -420, -360, -300, -240, -180, 0, 60, 120, 180,
                              240, 300, 360, 420, 480, 540, 600].choose(&mut rng).unwrap().clone();

        let hardware_concurrency = [2, 4, 6, 8, 12, 16].choose(&mut rng).unwrap().clone();

        let memory_gb = [4, 8, 16, 32].choose(&mut rng).unwrap().clone();

        let (webgl_renderer, webgl_vendor) = WEBGL_CONFIGS.choose(&mut rng).unwrap();

        let mut preferred_languages = vec![LANGUAGES.choose(&mut rng).unwrap().to_string()];
        if rng.gen_bool(0.3) {
            preferred_languages.push("en-US,en;q=0.9".to_string());
        }

        let mut instance = Self {
            user_agent: String::new(),
            accept_language: preferred_languages[0].clone(),
            platform: browser.platform.to_string(),
            viewport: *viewport,
            color_depth,
            pixel_ratio,
            timezone_offset,
            hardware_concurrency,
            memory_gb,
            webgl_vendor: webgl_vendor.to_string(),
            webgl_renderer: webgl_renderer.to_string(),
            architecture: ARCHITECTURES.choose(&mut rng).unwrap(),
            bitness: if browser.platform == "Windows" { "64" } else { "32" },
            platform_version: PLATFORM_VERSIONS.choose(&mut rng).unwrap().to_string(),
            browser_version: format!("{}.{}", browser.min_version, browser.max_version),
            mobile: false,
            headers: Vec::with_capacity(20),
            connection_type: CONNECTION_TYPES.choose(&mut rng).unwrap(),
            preferred_languages,
        };

        instance.user_agent = instance.generate_user_agent(&browser);
        instance.generate_headers(&browser);

        instance
    }

    pub fn apply_to_headers(&self, headers: &mut Headers) -> worker::Result<()> {
        headers.set("User-Agent", &self.user_agent)?;
        headers.set("Accept-Language", &self.accept_language)?;

        headers.set("Sec-CH-UA-Platform-Version", &self.platform_version)?;
        headers.set("Sec-CH-UA-Model", "")?;
        headers.set("Sec-CH-UA-Mobile", if self.mobile { "?1" } else { "?0" })?;

        headers.set("Viewport-Width", &self.viewport.0.to_string())?;
        headers.set("Width", &self.viewport.0.to_string())?;
        headers.set("Device-Memory", &self.memory_gb.to_string())?;
        headers.set("Sec-CH-UA-Full-Version", &self.browser_version)?;

        headers.set("Sec-CH-UA-WebGL-Vendor", &self.webgl_vendor)?;
        headers.set("Sec-CH-UA-WebGL-Renderer", &self.webgl_renderer)?;

        headers.set("Sec-CH-UA-Arch", self.architecture)?;
        headers.set("Sec-CH-UA-Bitness", self.bitness)?;

        headers.set("Downlink", "10.0")?;
        headers.set("ECT", self.connection_type)?;
        headers.set("RTT", "50")?;
        headers.set("Color-Depth", &self.color_depth.to_string())?;
        headers.set("Hardware-Concurrency", &self.hardware_concurrency.to_string())?;

        headers.set("Time-Zone", &self.timezone_offset.to_string())?;

        let langs = self.preferred_languages.join(",");
        headers.set("Accept-Language", &langs)?;

        for (name, value) in &self.headers {
            headers.set(name, value)?;
        }

        Ok(())
    }

    fn generate_user_agent(&self, browser: &BrowserVersion) -> String {
        match browser.name {
            "Chrome" => format!(
                "Mozilla/5.0 ({}) {} {} {} Safari/537.36",
                if self.platform == "Windows" {
                    format!("Windows NT {}; Win64; x64", self.platform_version)
                } else {
                    format!("Macintosh; Intel Mac OS X {}", self.platform_version)
                },
                browser.engine,
                browser.version_prefix,
                browser.min_version
            ),
            "Firefox" => format!(
                "Mozilla/5.0 ({}) Gecko/{} Firefox/{}",
                if self.platform == "Windows" {
                    format!("Windows NT {}; Win64; x64", self.platform_version)
                } else {
                    format!("Macintosh; Intel Mac OS X {}", self.platform_version)
                },
                browser.engine_version,
                browser.min_version
            ),
            "Safari" => format!(
                "Mozilla/5.0 ({}) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/{} Safari/605.1.15",
                if self.platform == "Windows" {
                    format!("Windows NT {}; Win64; x64", self.platform_version)
                } else {
                    format!("Macintosh; Intel Mac OS X {}", self.platform_version)
                },
                browser.min_version
            ),
            _ => unreachable!(),
        }
    }

    fn generate_headers(&mut self, browser: &BrowserVersion) {
        self.headers.clear();

        self.headers.push(("User-Agent".to_string(), self.user_agent.clone()));
        self.headers.push(("Accept-Language".to_string(), self.accept_language.clone()));
        self.headers.push(("Accept".to_string(), "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8".to_string()));
        self.headers.push(("Accept-Encoding".to_string(), "gzip, deflate, br".to_string()));
        self.headers.push(("Connection".to_string(), "keep-alive".to_string()));

        if browser.name == "Chrome" {
            self.headers.push(("Sec-CH-UA".to_string(), browser.brand_version.clone()));
            self.headers.push(("Sec-CH-UA-Mobile".to_string(), "?0".to_string()));
            self.headers.push(("Sec-CH-UA-Platform".to_string(), format!("\"{}\"", self.platform)));
            self.headers.push(("Sec-CH-UA-Arch".to_string(), format!("\"{}\"", self.architecture)));
            self.headers.push(("Sec-CH-UA-Bitness".to_string(), format!("\"{}\"", self.bitness)));
            self.headers.push(("Sec-CH-UA-Full-Version-List".to_string(), browser.brand_version.clone()));
            self.headers.push(("Device-Memory".to_string(), format!("{}", self.memory_gb)));
            self.headers.push(("Sec-CH-UA-Model".to_string(), "".to_string()));
            self.headers.push(("Color-Depth".to_string(), self.color_depth.to_string()));
            self.headers.push(("Hardware-Concurrency".to_string(), self.hardware_concurrency.to_string()));

        }

        self.headers.push(("Viewport-Width".to_string(), self.viewport.0.to_string()));
        self.headers.push(("DPR".to_string(), format!("{:.1}", self.pixel_ratio)));
        self.headers.push(("Device-Memory".to_string(), format!("{}", self.memory_gb)));
        self.headers.push(("RTT".to_string(), "50".to_string()));
        self.headers.push(("Downlink".to_string(), "10.0".to_string()));
        self.headers.push(("ECT".to_string(), self.connection_type.to_string()));
    }
}

pub struct FingerprintCache {
    pub fingerprints: Vec<BrowserFingerprint>,
}

impl FingerprintCache {
    pub fn new() -> Self {
        let mut fingerprints = Vec::with_capacity(10);
        for _ in 0..10 {
            fingerprints.push(BrowserFingerprint::generate());
        }
        Self { fingerprints }
    }

    pub fn get_random(&self) -> BrowserFingerprint {
        let now = Date::now() as u64;
        let mut rng = SmallRng::seed_from_u64(now);
        if rng.gen_bool(0.1) {
            BrowserFingerprint::generate()
        } else {
            self.fingerprints.choose(&mut rng).unwrap().clone()
        }
    }
}