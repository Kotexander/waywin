use pollster::FutureExt;
use utils::ColorClearer;
use waywin::event::Event;
mod utils;

fn run(title: &str) {
    let waywin = waywin::init(title).unwrap();
    let window = waywin.create_window(title).unwrap();

    let mut color_clearer = ColorClearer::new(&window).block_on().unwrap();

    color_clearer.clear();
    window.show();

    waywin.run(|event| {
        // log::info!("{:?}", event);
        match event.kind {
            Event::Close => {
                waywin.exit();
                // window.hide();
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
}

fn main() {
    colog::init();

    // std::thread::scope(|s| {
    //     s.spawn(|| run("thread 1"));
    //     s.spawn(|| run("thread 2"));
    // });

    let h1 = std::thread::spawn(|| run("thread 1"));
    // let h2 = std::thread::spawn(|| run("thread 2"));

    h1.join().unwrap();
    // h2.join().unwrap();
}
