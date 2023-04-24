use std::sync::{Arc, RwLock};

use typst::geom::Abs;
use typst_ts_core::{font::FontResolverImpl, Artifact};
use wasm_bindgen::prelude::*;

use crate::renderer::artifact::artifact_from_js_string;

#[wasm_bindgen]
pub struct RenderSessionOptions {
    pub(crate) pixel_per_pt: Option<f32>,
    pub(crate) background_color: Option<String>,
}

#[wasm_bindgen]
impl RenderSessionOptions {
    #[wasm_bindgen(constructor)]
    pub fn new() -> RenderSessionOptions {
        Self {
            pixel_per_pt: None,
            background_color: None,
        }
    }

    #[wasm_bindgen(getter)]
    pub fn pixel_per_pt(&self) -> Option<f32> {
        self.pixel_per_pt.clone()
    }

    #[wasm_bindgen(setter)]
    pub fn set_pixel_per_pt(&mut self, pixel_per_pt: f32) {
        self.pixel_per_pt = Some(pixel_per_pt);
    }

    #[wasm_bindgen(getter)]
    pub fn background_color(&self) -> Option<String> {
        self.background_color.clone()
    }

    #[wasm_bindgen(setter)]
    pub fn set_background_color(&mut self, background_color: String) {
        self.background_color = Some(background_color);
    }
}

#[wasm_bindgen]
#[derive(Clone)]
pub struct PageInfo {
    pub(crate) width: Abs,
    pub(crate) height: Abs,
}

#[wasm_bindgen]
impl PageInfo {
    #[wasm_bindgen(getter)]
    pub fn width_pt(&self) -> f64 {
        self.width.to_pt()
    }

    #[wasm_bindgen(getter)]
    pub fn height_pt(&self) -> f64 {
        self.height.to_pt()
    }
}

#[wasm_bindgen]
#[derive(Clone)]
pub struct PagesInfo {
    pub(crate) pages: Vec<PageInfo>,
}

#[wasm_bindgen]
impl PagesInfo {
    #[wasm_bindgen(getter)]
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    pub fn page(&self, i: usize) -> PageInfo {
        self.pages[i].clone()
    }
}

#[wasm_bindgen]
pub struct RenderSession {
    pub(crate) pixel_per_pt: f32,
    pub(crate) background_color: String,
    pub(crate) doc: typst::doc::Document,
    pub(crate) pages_info: PagesInfo,
}

#[wasm_bindgen]
impl RenderSession {
    #[wasm_bindgen(getter)]
    pub fn pixel_per_pt(&self) -> f32 {
        self.pixel_per_pt.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn background_color(&self) -> String {
        self.background_color.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn pages_info(&self) -> PagesInfo {
        self.pages_info.clone()
    }
}

impl RenderSession {
    pub(crate) fn from_doc(doc: typst::doc::Document) -> Self {
        let pages_info = PagesInfo {
            pages: {
                let mut pages = Vec::new();
                pages.reserve(doc.pages.len());
                for page in doc.pages.iter() {
                    pages.push(PageInfo {
                        width: page.size().x,
                        height: page.size().y,
                    });
                }
                pages
            },
        };

        Self {
            pixel_per_pt: 0.,
            background_color: "".to_string(),
            doc,
            pages_info,
        }
    }
}

#[wasm_bindgen]
pub struct RenderSessionManager {
    font_resolver: Arc<RwLock<FontResolverImpl>>,
}

#[wasm_bindgen]
impl RenderSessionManager {
    pub fn create_session(
        &self,
        artifact_content: String,
        options: Option<RenderSessionOptions>,
    ) -> Result<RenderSession, JsValue> {
        let mut ses = self.session_from_artifact(artifact_content, "js")?;

        ses.pixel_per_pt = options
            .as_ref()
            .and_then(|o| o.pixel_per_pt.clone())
            .unwrap_or(2.);

        ses.background_color = options
            .as_ref()
            .and_then(|o| o.background_color.clone())
            .unwrap_or("ffffff".to_string());

        Ok(ses)
    }
}

impl RenderSessionManager {
    pub fn new(fr: FontResolverImpl) -> Self {
        Self {
            font_resolver: Arc::new(RwLock::new(fr)),
        }
    }

    pub fn session_from_artifact(
        &self,
        artifact_content: String,
        decoder: &str,
    ) -> Result<RenderSession, JsValue> {
        // 550KB -> 147KB
        // https://medium.com/@wl1508/avoiding-using-serde-and-deserde-in-rust-webassembly-c1e4640970ca
        let artifact: Artifact = match decoder {
            "js" => {
                let artifact: Artifact = artifact_from_js_string(artifact_content)?;

                artifact
            }

            #[cfg(feature = "serde")]
            "serde" => {
                let artifact: Artifact = serde_json::from_str(artifact_content.as_str()).unwrap();

                artifact
            }
            _ => {
                panic!("unknown decoder: {}", decoder);
            }
        };

        #[cfg(debug)]
        {
            use super::utils::console_log;
            use web_sys::console;
            let _ = console::log_0;
            console_log!(
                "{} pages to render. font info: {:?}",
                artifact.pages.len(),
                artifact
                    .fonts
                    .iter()
                    .map(|f| f.family.as_str()) // serde_json::to_string(f).unwrap())
                    .collect::<Vec<&str>>()
                    .join(", ")
            );
        }

        let font_resolver = self.font_resolver.read().unwrap();
        let document = artifact.to_document(&*font_resolver);
        if document.pages.len() == 0 {
            return Err("no pages in artifact".into());
        }

        Ok(RenderSession::from_doc(document))
    }
}