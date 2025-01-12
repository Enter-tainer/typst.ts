//! Lowering Typst Document into SvgItem.

use std::io::Read;
use std::sync::Arc;

use once_cell::sync::OnceCell;
use typst::doc::{Destination, Document, Frame, FrameItem, GroupItem, Meta, Position, TextItem};
use typst::font::Font;
use typst::geom::{Dir, FixedStroke, Geometry, LineCap, LineJoin, Paint, PathItem, Shape, Size};
use typst::image::Image;

use ttf_parser::OutlineBuilder;
use typst::model::Introspector;
use typst::syntax::Span;

use super::{
    geom::Scalar,
    ir::{self, GlyphItem, ImageGlyphItem, OutlineGlyphItem, SvgItem, TransformItem},
};
use super::{
    path2d::SvgPath2DBuilder,
    sk,
    utils::{AbsExt, ToCssExt},
};
use crate::font::GlyphProvider;
use ttf_parser::GlyphId;

static WARN_VIEW_BOX: OnceCell<()> = OnceCell::new();

/// Lower a frame item into svg item.
pub struct LowerBuilder {
    introspector: Introspector,
}

impl LowerBuilder {
    pub fn new(output: &Document) -> Self {
        Self {
            introspector: Introspector::new(&output.pages),
        }
    }

    /// Lower a frame into svg item.
    pub fn lower(&mut self, frame: &Frame) -> SvgItem {
        self.lower_frame(frame)
    }

    /// Lower a frame into svg item.
    fn lower_frame(&mut self, frame: &Frame) -> SvgItem {
        let mut items = Vec::with_capacity(frame.items().len());

        for (pos, item) in frame.items() {
            let item = match item {
                FrameItem::Group(group) => self.lower_group(group),
                FrameItem::Text(text) => Self::lower_text(text),
                FrameItem::Shape(shape, span_id) => {
                    SvgItem::Path((Self::lower_shape(shape), hack_span_id_to_u64(span_id)))
                }
                FrameItem::Image(image, size, span_id) => {
                    SvgItem::Image((lower_image(image, *size), hack_span_id_to_u64(span_id)))
                }
                FrameItem::Meta(meta, size) => match meta {
                    Meta::Link(lnk) => match lnk {
                        Destination::Url(url) => self.lower_link(url, *size),
                        Destination::Position(dest) => Self::lower_position(*dest, *size),
                        Destination::Location(loc) => {
                            // todo: process location before lowering
                            let dest = self.introspector.position(*loc);
                            Self::lower_position(dest, *size)
                        }
                    },
                    // todo: support page label
                    Meta::PdfPageLabel(..)
                    | Meta::Elem(..)
                    | Meta::PageNumbering(..)
                    | Meta::Hide => continue,
                },
            };

            items.push(((*pos).into(), item));
        }

        // swap link items
        items.sort_by(|x, y| {
            let x_is_link = matches!(x.1, SvgItem::Link(..));
            let y_is_link = matches!(y.1, SvgItem::Link(..));
            if x_is_link || y_is_link {
                if x_is_link && y_is_link {
                    return std::cmp::Ordering::Equal;
                } else if x_is_link {
                    return std::cmp::Ordering::Greater;
                } else {
                    return std::cmp::Ordering::Less;
                }
            }

            std::cmp::Ordering::Equal
        });

        SvgItem::Group(ir::GroupItem(items))
    }

    /// Lower a group frame with optional transform and clipping into svg item.
    fn lower_group(&mut self, group: &GroupItem) -> SvgItem {
        let mut inner = self.lower_frame(&group.frame);

        if group.clips {
            let mask_box = {
                let mut builder = SvgPath2DBuilder::default();

                // build a rectangle path
                let size = group.frame.size();
                let w = size.x.to_f32();
                let h = size.y.to_f32();
                builder.rect(0., 0., w, h);

                builder.0
            };

            inner = SvgItem::Transformed(ir::TransformedItem(
                TransformItem::Clip(Arc::new(ir::PathItem {
                    d: mask_box.into(),
                    styles: vec![],
                })),
                Box::new(inner),
            ));
        };

        SvgItem::Transformed(ir::TransformedItem(
            TransformItem::Matrix(Arc::new(group.transform.into())),
            Box::new(inner),
        ))
    }

    /// Lower a link into svg item.
    pub(super) fn lower_link(&self, url: &str, size: Size) -> ir::SvgItem {
        SvgItem::Link(ir::LinkItem {
            href: url.into(),
            size: size.into(),
        })
    }

    /// Lower a document position into svg item.
    #[comemo::memoize]
    pub(super) fn lower_position(pos: Position, size: Size) -> ir::SvgItem {
        let lnk = ir::LinkItem {
            href: format!(
                "@typst:handleTypstLocation(this, {}, {}, {})",
                pos.page,
                pos.point.x.to_f32(),
                pos.point.y.to_f32()
            )
            .into(),
            size: size.into(),
        };

        SvgItem::Link(lnk)
    }

    /// Lower a text into svg item.
    // #[comemo::memoize]
    pub(super) fn lower_text(text: &TextItem) -> SvgItem {
        let mut glyphs = Vec::with_capacity(text.glyphs.len());
        for glyph in &text.glyphs {
            let id = GlyphId(glyph.id);
            glyphs.push((
                glyph.x_offset.at(text.size).into(),
                glyph.x_advance.at(text.size).into(),
                ir::GlyphItem::Raw(text.font.clone(), id),
            ));
        }

        let glyph_chars: String = text.text.to_string();

        let Paint::Solid(fill) = text.fill;
        let fill = fill.to_css().into();

        let span_id = text
            .glyphs
            .iter()
            .filter(|g| g.span.0 != Span::detached())
            .map(|g| &g.span.0)
            .map(hack_span_id_to_u64)
            .max()
            .unwrap_or_else(|| hack_span_id_to_u64(&Span::detached()));

        SvgItem::Text(ir::TextItem {
            font: text.font.clone(),
            content: Arc::new(ir::TextItemContent {
                content: glyph_chars.into(),
                glyphs,
                span_id,
            }),
            shape: Arc::new(ir::TextShape {
                size: Scalar(text.size.to_f32()),
                dir: match text.lang.dir() {
                    Dir::LTR => "ltr",
                    Dir::RTL => "rtl",
                    Dir::TTB => "ttb",
                    Dir::BTT => "btt",
                }
                .into(),
                fill,
            }),
        })
    }

    /// Lower a geometrical shape into svg item.
    #[comemo::memoize]
    pub(super) fn lower_shape(shape: &Shape) -> ir::PathItem {
        let mut builder = SvgPath2DBuilder(String::new());

        // to ensure that our shape focus on the original point
        builder.move_to(0., 0.);
        match shape.geometry {
            Geometry::Line(target) => {
                builder.line_to(target.x.to_f32(), target.y.to_f32());
            }
            Geometry::Rect(size) => {
                let w = size.x.to_f32();
                let h = size.y.to_f32();
                builder.line_to(0., h);
                builder.line_to(w, h);
                builder.line_to(w, 0.);
                builder.close();
            }
            Geometry::Path(ref path) => {
                for elem in &path.0 {
                    match elem {
                        PathItem::MoveTo(p) => {
                            builder.move_to(p.x.to_f32(), p.y.to_f32());
                        }
                        PathItem::LineTo(p) => {
                            builder.line_to(p.x.to_f32(), p.y.to_f32());
                        }
                        PathItem::CubicTo(p1, p2, p3) => {
                            builder.curve_to(
                                p1.x.to_f32(),
                                p1.y.to_f32(),
                                p2.x.to_f32(),
                                p2.y.to_f32(),
                                p3.x.to_f32(),
                                p3.y.to_f32(),
                            );
                        }
                        PathItem::ClosePath => {
                            builder.close();
                        }
                    };
                }
            }
        };

        let d = builder.0.into();

        let mut styles = Vec::new();

        if let Some(paint_fill) = &shape.fill {
            let Paint::Solid(color) = paint_fill;
            let c = color.to_css();

            styles.push(ir::PathStyle::Fill(c.into()));
        }

        // todo: default miter_limit, thickness
        if let Some(FixedStroke {
            paint,
            thickness,
            line_cap,
            line_join,
            dash_pattern,
            miter_limit,
        }) = &shape.stroke
        {
            if let Some(pattern) = dash_pattern.as_ref() {
                styles.push(ir::PathStyle::StrokeDashOffset(pattern.phase.into()));
                let d = pattern.array.clone();
                let d = d.into_iter().map(Scalar::from).collect();
                styles.push(ir::PathStyle::StrokeDashArray(d));
            }

            styles.push(ir::PathStyle::StrokeWidth((*thickness).into()));
            styles.push(ir::PathStyle::StrokeMitterLimit((*miter_limit).into()));
            match line_cap {
                LineCap::Butt => {}
                LineCap::Round => styles.push(ir::PathStyle::StrokeLineCap("round".into())),
                LineCap::Square => styles.push(ir::PathStyle::StrokeLineCap("square".into())),
            };
            match line_join {
                LineJoin::Miter => {}
                LineJoin::Bevel => styles.push(ir::PathStyle::StrokeLineJoin("bevel".into())),
                LineJoin::Round => styles.push(ir::PathStyle::StrokeLineJoin("round".into())),
            }

            let Paint::Solid(color) = paint;
            styles.push(ir::PathStyle::Stroke(color.to_css().into()));
        }

        ir::PathItem { d, styles }
    }
}

/// Lower a glyph into svg item.
pub struct GlyphLowerBuilder<'a> {
    gp: &'a GlyphProvider,
}

impl<'a> GlyphLowerBuilder<'a> {
    pub fn new(gp: &'a GlyphProvider) -> Self {
        Self { gp }
    }

    pub fn lower_glyph(&self, glyph_item: &GlyphItem) -> Option<GlyphItem> {
        match glyph_item {
            GlyphItem::Raw(font, id) => {
                let id = *id;
                self.lower_svg_glyph(font, id)
                    .map(GlyphItem::Image)
                    .or_else(|| self.lower_bitmap_glyph(font, id).map(GlyphItem::Image))
                    .or_else(|| self.lower_outline_glyph(font, id).map(GlyphItem::Outline))
            }
            GlyphItem::Image(..) | GlyphItem::Outline(..) => Some(glyph_item.clone()),
            GlyphItem::None => Some(GlyphItem::None),
        }
    }

    /// Lower an SVG glyph into svg item.
    /// More information: https://learn.microsoft.com/zh-cn/typography/opentype/spec/svg
    fn lower_svg_glyph(&self, font: &Font, id: GlyphId) -> Option<Arc<ImageGlyphItem>> {
        let glyph_image = extract_svg_glyph(self.gp, font, id)?;

        let sz = Size::new(
            typst::geom::Abs::raw(glyph_image.width() as f64),
            typst::geom::Abs::raw(glyph_image.height() as f64),
        );

        let image = ir::ImageItem {
            image: Arc::new(glyph_image.into()),
            size: sz.into(),
        };

        // position our image
        let ascender = font
            .metrics()
            .ascender
            .at(typst::geom::Abs::raw(font.metrics().units_per_em))
            .to_f32();

        Some(Arc::new(ImageGlyphItem {
            ts: ir::Transform {
                sx: Scalar(1.),
                ky: Scalar(0.),
                kx: Scalar(0.),
                sy: Scalar(-1.),
                tx: Scalar(0.),
                ty: Scalar(ascender),
            },
            image,
        }))
    }

    /// Lower a bitmap glyph into the svg text.
    fn lower_bitmap_glyph(&self, font: &Font, id: GlyphId) -> Option<Arc<ImageGlyphItem>> {
        let ppem = u16::MAX;
        let upem = font.metrics().units_per_em as f32;

        let (glyph_image, raster_x, raster_y) = self.gp.bitmap_glyph(font, id, ppem)?;

        // FIXME: Vertical alignment isn't quite right for Apple Color Emoji,
        // and maybe also for Noto Color Emoji. And: Is the size calculation
        // correct?

        let w = glyph_image.width() as f64;
        let h = glyph_image.height() as f64;
        let sz = Size::new(typst::geom::Abs::raw(w), typst::geom::Abs::raw(h));

        let image = ir::ImageItem {
            image: Arc::new(glyph_image.into()),
            size: sz.into(),
        };

        // position our image
        // first, the ascender is used
        // next, also apply an offset of (1 - ascender) like typst
        let adjusted = font.metrics().ascender * 2. - typst::geom::Em::one();
        // let adjusted = font.metrics().ascender;

        let adjusted = adjusted
            .at(typst::geom::Abs::raw(font.metrics().units_per_em))
            .to_f32();

        let ts = sk::Transform::from_scale(upem / w as f32, -upem / h as f32);

        // size
        let dx = raster_x as f32;
        let dy = raster_y as f32;
        let ts = ts.post_translate(dx, adjusted + dy);

        Some(Arc::new(ImageGlyphItem {
            ts: ts.into(),
            image,
        }))
    }

    /// Lower an outline glyph into svg text. This is the "normal" case.
    fn lower_outline_glyph(&self, font: &Font, id: GlyphId) -> Option<Arc<OutlineGlyphItem>> {
        let d = self.gp.outline_glyph(font, id)?.into();

        Some(Arc::new(OutlineGlyphItem { ts: None, d }))
    }
}

fn extract_svg_glyph(g: &GlyphProvider, font: &Font, id: GlyphId) -> Option<typst::image::Image> {
    let data = g.svg_glyph(font, id)?;
    let mut data = data.as_ref();

    let font_metrics = font.metrics();

    // Decompress SVGZ.
    let mut decoded = vec![];

    // The first three bytes of the gzip-encoded document header must be
    //   0x1F, 0x8B, 0x08.
    if data.starts_with(&[0x1f, 0x8b]) {
        let mut decoder = flate2::read::GzDecoder::new(data);
        decoder.read_to_end(&mut decoded).ok()?;
        data = &decoded;
    }

    // todo: It is also legal to provide a SVG document containing multiple glyphs.
    // > When a font engine renders glyph 14, the result shall be the same as
    // > rendering the following SVG document:
    // > `  <svg> <defs> <use #glyph{id}> </svg>`
    // See: <https://learn.microsoft.com/en-us/typography/opentype/spec/svg#glyph-identifiers>

    let upem = typst::geom::Abs::raw(font.units_per_em());
    let (width, height) = (upem.to_f32(), upem.to_f32());
    let origin_ascender = font_metrics.ascender.at(upem).to_f32();

    let doc_string = String::from_utf8(data.to_owned()).unwrap();

    // todo: verify SVG capability requirements and restrictions

    // Partially parse the view box attribute
    let mut svg_str = std::str::from_utf8(data).ok()?.to_owned();
    let FindViewBoxResult {
        start_span,
        first_viewbox,
    } = find_viewbox_attr(svg_str.as_str());

    // determine view box
    let view_box = first_viewbox.as_ref()
        .map(|s| {
            WARN_VIEW_BOX.get_or_init(|| {
                println!(
                    "render_svg_glyph with viewBox, This should be helpful if you can help us verify the result: {:?} {:?}",
                    font.info().family,
                    doc_string
                );
            });
            s.1.as_str().to_owned()
        })
        .unwrap_or_else(|| format!("0 {} {width} {height}", -origin_ascender));

    // determine view box
    match first_viewbox {
        Some((span, ..)) => {
            // replace the first viewBox attribute
            svg_str.replace_range(span.range(), format!(r#"viewBox="{view_box}""#).as_str());
        }
        None => {
            // insert viewBox attribute to the begin of svg tag
            svg_str.insert_str(
                start_span.unwrap().range().end,
                format!(r#" viewBox="{view_box}""#).as_str(),
            );
        }
    }

    let image = typst::image::Image::new_raw(
        svg_str.as_bytes().to_vec().into(),
        typst::image::ImageFormat::Vector(typst::image::VectorFormat::Svg),
        typst::geom::Axes::new(width as u32, height as u32),
        None,
    )
    .ok()?;

    Some(image)
}

/// Lower a raster or SVG image into svg item.
#[comemo::memoize]
fn lower_image(image: &Image, size: Size) -> ir::ImageItem {
    ir::ImageItem {
        image: Arc::new(image.clone().into()),
        size: size.into(),
    }
}

fn hack_span_id_to_u64(span_id: &Span) -> u64 {
    const SPAN_BITS: u64 = 48;
    // todo: how to get file_id?
    let file_id = unsafe { std::mem::transmute::<_, &u16>(&span_id.id()) };
    ((*file_id as u64) << SPAN_BITS) | span_id.number()
}

struct FindViewBoxResult<'a> {
    start_span: Option<xmlparser::StrSpan<'a>>,
    first_viewbox: Option<(xmlparser::StrSpan<'a>, xmlparser::StrSpan<'a>)>,
}

/// Find the string location of the **first** viewBox attribute.
/// When there are multiple viewBox attributes, the first one is used (as many
/// xml-based dom engines do).
fn find_viewbox_attr(svg_str: &'_ str) -> FindViewBoxResult<'_> {
    let document = xmlparser::Tokenizer::from(svg_str);

    let mut start_span = None;
    let mut first_viewbox = None;
    for n in document {
        let tok = n.unwrap();
        match tok {
            xmlparser::Token::ElementStart { span, local, .. } => {
                if local.as_str() == "svg" {
                    start_span = Some(span);
                }
            }
            xmlparser::Token::Attribute {
                span, local, value, ..
            } => {
                if local.as_str() == "viewBox" {
                    first_viewbox = Some((span, value));
                    break;
                }
            }
            xmlparser::Token::ElementEnd { .. } => break,
            _ => {}
        }
    }

    FindViewBoxResult {
        start_span,
        first_viewbox,
    }
}
