use anyhow::Result;
use lopdf::{Document, Object, ObjectId};
use tracing::debug;

/// Decorative figure: area < 2% of page AND near page edges.
const DECORATIVE_AREA_RATIO: f32 = 0.02;
/// Edge margin for decorative detection: 5% of smaller page dimension.
const DECORATIVE_EDGE_MARGIN_RATIO: f32 = 0.05;

/// An image extracted from a PDF page's XObject resources.
#[derive(Debug, Clone)]
pub struct ExtractedPdfImage {
    pub object_id: (u32, u16),
    pub width: i64,
    pub height: i64,
    pub content_type: PdfImageFormat,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PdfImageFormat {
    Jpeg,
    Png,
    Raw,
}

/// A figure's position and size on the page (in page-space units).
#[derive(Debug, Clone)]
pub struct FigurePlacement {
    pub xobject_name: String,
    pub bbox: [f32; 4], // [x, y, width, height] in page units
    pub area: f32,
    pub is_decorative: bool,
}

/// Compute figure area ratio for a page by analyzing the content stream.
/// Returns (figure_area_ratio, non_decorative_figure_count, figure_placements).
pub fn compute_figure_area_ratio(
    doc: &Document,
    page_id: ObjectId,
) -> Result<(f32, usize, Vec<FigurePlacement>)> {
    let page_obj = doc.get_object(page_id)?;
    let page_dict = page_obj.as_dict()?;

    // Get MediaBox for page dimensions
    let (page_w, page_h) = match page_dict.get(b"MediaBox") {
        Ok(obj) => {
            let arr = obj.as_array()?;
            if arr.len() >= 4 {
                let x0 = object_to_f32(&arr[0]);
                let y0 = object_to_f32(&arr[1]);
                let x1 = object_to_f32(&arr[2]);
                let y1 = object_to_f32(&arr[3]);
                ((x1 - x0).abs(), (y1 - y0).abs())
            } else {
                return Ok((0.0, 0, Vec::new()));
            }
        }
        Err(_) => return Ok((0.0, 0, Vec::new())),
    };
    let page_area = page_w * page_h;
    if page_area <= 0.0 {
        return Ok((0.0, 0, Vec::new()));
    }

    // Get Image XObject names from Resources
    let image_xobject_names = get_image_xobject_names(doc, page_id)?;

    // Parse content stream
    let content = doc.get_and_decode_page_content(page_id)?;
    let mut ctm_stack: Vec<[f32; 6]> = Vec::new();
    let mut ctm: [f32; 6] = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]; // identity
    let mut placements = Vec::new();

    for op in content.operations.iter() {
        match op.operator.as_str() {
            "cm" => {
                // Concatenate matrix: [a b c d e f]
                if op.operands.len() >= 6 {
                    let m = [
                        object_to_f32(&op.operands[0]),
                        object_to_f32(&op.operands[1]),
                        object_to_f32(&op.operands[2]),
                        object_to_f32(&op.operands[3]),
                        object_to_f32(&op.operands[4]),
                        object_to_f32(&op.operands[5]),
                    ];
                    ctm = multiply_matrices(ctm, m);
                }
            }
            "q" => {
                ctm_stack.push(ctm);
            }
            "Q" => {
                if let Some(saved) = ctm_stack.pop() {
                    ctm = saved;
                }
            }
            "Do" => {
                if let Some(name_obj) = op.operands.first() {
                    let name = match name_obj {
                        Object::Name(n) => String::from_utf8_lossy(n).to_string(),
                        _ => continue,
                    };
                    if image_xobject_names.contains(&name) {
                        // CTM maps unit square [0,1]×[0,1] to page space
                        // The transformed unit square gives us the figure bbox
                        let p0 = transform_point(&ctm, 0.0, 0.0);
                        let p1 = transform_point(&ctm, 1.0, 0.0);
                        let p2 = transform_point(&ctm, 1.0, 1.0);
                        let p3 = transform_point(&ctm, 0.0, 1.0);

                        let min_x = p0.0.min(p1.0).min(p2.0).min(p3.0);
                        let max_x = p0.0.max(p1.0).max(p2.0).max(p3.0);
                        let min_y = p0.1.min(p1.1).min(p2.1).min(p3.1);
                        let max_y = p0.1.max(p1.1).max(p2.1).max(p3.1);

                        let w = (max_x - min_x).abs();
                        let h = (max_y - min_y).abs();
                        let area = w * h;

                        // Decorative: near page edges AND small relative to page
                        let edge_margin = page_w.min(page_h) * DECORATIVE_EDGE_MARGIN_RATIO;
                        let near_edge = min_x < edge_margin
                            || min_y < edge_margin
                            || max_x > page_w - edge_margin
                            || max_y > page_h - edge_margin;
                        let small = area < page_area * DECORATIVE_AREA_RATIO;
                        let is_decorative = near_edge && small;

                        placements.push(FigurePlacement {
                            xobject_name: name,
                            bbox: [min_x, min_y, w, h],
                            area,
                            is_decorative,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    let total_figure_area: f32 = placements.iter().map(|p| p.area).sum();
    let figure_area_ratio = (total_figure_area / page_area).min(1.0);
    let non_decorative_count = placements.iter().filter(|p| !p.is_decorative).count();

    Ok((figure_area_ratio, non_decorative_count, placements))
}

fn get_image_xobject_names(doc: &Document, page_id: ObjectId) -> Result<Vec<String>> {
    let page_obj = doc.get_object(page_id)?;
    let page_dict = page_obj.as_dict()?;
    let resources = match page_dict.get(b"Resources") {
        Ok(r) => r.as_dict()?,
        Err(_) => return Ok(Vec::new()),
    };
    let xobjects = match resources.get(b"XObject") {
        Ok(x) => x.as_dict()?,
        Err(_) => return Ok(Vec::new()),
    };

    let mut names = Vec::new();
    for (name, obj_ref) in xobjects.iter() {
        let reference = match obj_ref.as_reference() {
            Ok(r) => r,
            Err(_) => continue,
        };
        let xobj = match doc.get_object(reference) {
            Ok(o) => o,
            Err(_) => continue,
        };
        let xobj_dict = match xobj.as_dict() {
            Ok(d) => d,
            Err(_) => continue,
        };
        if let Ok(subtype) = xobj_dict.get(b"Subtype") {
            if let Ok(name_bytes) = subtype.as_name() {
                if name_bytes == b"Image" {
                    names.push(String::from_utf8_lossy(name).to_string());
                }
            }
        }
    }
    Ok(names)
}

fn transform_point(ctm: &[f32; 6], x: f32, y: f32) -> (f32, f32) {
    let nx = ctm[0] * x + ctm[2] * y + ctm[4];
    let ny = ctm[1] * x + ctm[3] * y + ctm[5];
    (nx, ny)
}

fn multiply_matrices(a: [f32; 6], b: [f32; 6]) -> [f32; 6] {
    [
        a[0] * b[0] + a[2] * b[1],
        a[1] * b[0] + a[3] * b[1],
        a[0] * b[2] + a[2] * b[3],
        a[1] * b[2] + a[3] * b[3],
        a[0] * b[4] + a[2] * b[5] + a[4],
        a[1] * b[4] + a[3] * b[5] + a[5],
    ]
}

fn object_to_f32(obj: &Object) -> f32 {
    match obj {
        Object::Integer(i) => *i as f32,
        Object::Real(f) => *f,
        _ => 0.0,
    }
}

/// Extract Image XObjects from a specific PDF page.
/// Returns images with their raw data (JPEG if DCTDecode, decompressed otherwise).
pub fn extract_page_images(bytes: &[u8], page_number: u32) -> Result<Vec<ExtractedPdfImage>> {
    let doc = Document::load_mem(bytes)?;
    let pages = doc.get_pages();
    let page_id = pages
        .get(&page_number)
        .copied()
        .ok_or_else(|| anyhow::anyhow!("page {page_number} not found"))?;

    let pdf_images = doc.get_page_images(page_id)?;
    let mut result = Vec::with_capacity(pdf_images.len());

    for img in pdf_images {
        let format = detect_image_format(&img);
        let data = match format {
            PdfImageFormat::Jpeg => img.content.to_vec(),
            _ => {
                // For non-JPEG, try to decompress FlateDecode
                match decompress_stream(img.content) {
                    Ok(decompressed) => decompressed,
                    Err(_) => img.content.to_vec(),
                }
            }
        };

        // Skip tiny decorative images (< 50x50 or < 1KB)
        if img.width < 50 || img.height < 50 || data.len() < 1024 {
            debug!(
                page = page_number,
                width = img.width,
                height = img.height,
                size = data.len(),
                "skipping tiny/decorative image"
            );
            continue;
        }

        result.push(ExtractedPdfImage {
            object_id: img.id,
            width: img.width,
            height: img.height,
            content_type: format,
            data,
        });
    }

    Ok(result)
}

fn detect_image_format(img: &lopdf::xobject::PdfImage) -> PdfImageFormat {
    if let Some(filters) = &img.filters {
        if filters.iter().any(|f| f == "DCTDecode") {
            return PdfImageFormat::Jpeg;
        }
        if filters.iter().any(|f| f == "FlateDecode") {
            return PdfImageFormat::Png;
        }
    }
    PdfImageFormat::Raw
}

fn decompress_stream(data: &[u8]) -> Result<Vec<u8>> {
    use flate2::read::ZlibDecoder;
    use std::io::Read;
    let mut decoder = ZlibDecoder::new(data);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)?;
    Ok(decompressed)
}

/// Convert extracted image data to base64 for VLM API calls.
pub fn image_to_base64(img: &ExtractedPdfImage) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(&img.data)
}

/// Get the MIME type string for an extracted image.
pub fn image_mime_type(img: &ExtractedPdfImage) -> &'static str {
    match img.content_type {
        PdfImageFormat::Jpeg => "image/jpeg",
        PdfImageFormat::Png => "image/png",
        PdfImageFormat::Raw => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_format_detection() {
        // Just verify the enum variants exist
        assert_ne!(PdfImageFormat::Jpeg, PdfImageFormat::Png);
        assert_ne!(PdfImageFormat::Raw, PdfImageFormat::Jpeg);
    }

    #[test]
    fn test_mime_type() {
        let img = ExtractedPdfImage {
            object_id: (0, 0),
            width: 100,
            height: 100,
            content_type: PdfImageFormat::Jpeg,
            data: vec![0xFF, 0xD8],
        };
        assert_eq!(image_mime_type(&img), "image/jpeg");
    }
}
