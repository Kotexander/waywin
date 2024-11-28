use pollster::FutureExt;
use std::error::Error;
use utils::ColorClearer;
use waywin::event::Event;

mod utils;

fn main() -> Result<(), Box<dyn Error>> {
    colog::init();

    let waywin = waywin::init("hello_world")?;
    let window = waywin.create_window("Hello World")?;

    let mut color_clearer = ColorClearer::new(&window).block_on().unwrap();

    color_clearer.clear();
    window.show();

    waywin.run(|event| {
        log::info!("{event:?}");

        match event.kind {
            Event::Close => {
                waywin.exit();
                window.hide();
            }
            Event::Resize(w, h) => {
                color_clearer.resize(w, h);
            }
            Event::Paint => {
                color_clearer.clear();
                window.request_redraw();
            }
            _ => {}
        }
    });

    Ok(())
}
