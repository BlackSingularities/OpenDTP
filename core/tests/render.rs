use opendtp_core::{io, layout::Engine, paint, raster::Raster, sample};

#[test]
fn render_welcome() {
    let doc = sample::welcome();
    let mut eng = Engine::new();
    let t = std::time::Instant::now();
    let lay = eng.layout(&doc);
    eprintln!("skład: {:?}, strony: {}, wiersze: {}", t.elapsed(), lay.pages.len(), lay.lines.len());
    let t = std::time::Instant::now();
    let _ = eng.layout(&doc);
    eprintln!("ponowny skład (pamięć): {:?}", t.elapsed());
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/test-out");
    std::fs::create_dir_all(&out).unwrap();
    let mut r = Raster::new();
    let opts = paint::PaintOpts { screen: true, marks: false, guides: true, misspelled: None, hide_obj: None };
    for p in 0..lay.pages.len() {
        let scale = 1.5;
        let mut pm = tiny_skia::Pixmap::new((doc.page.width * scale) as u32, (doc.page.height * scale) as u32).unwrap();
        let ops = paint::page_ops(&doc, &lay, p, &opts);
        let t = std::time::Instant::now();
        r.draw(&doc, &ops, &mut pm, scale);
        eprintln!("strona {p}: {:?}", t.elapsed());
        pm.save_png(out.join(format!("page{p}.png"))).unwrap();
    }
    let docx = io::docx::export(&doc, &lay, &io::docx::DocxOpts { exact_shapes: true }).unwrap();
    std::fs::write(out.join("welcome.docx"), &docx).unwrap();
    let pdf = io::pdf::export(&doc, &lay).unwrap();
    std::fs::write(out.join("welcome.pdf"), &pdf).unwrap();
    let odtp = io::odtp::to_bytes(&doc).unwrap();
    let back = io::odtp::from_bytes(&odtp).unwrap();
    assert_eq!(back.body, doc.body);
    let again = io::docx_in::import(&docx).unwrap();
    assert!(again.body.paras.len() >= doc.body.paras.len() - 1);
}
