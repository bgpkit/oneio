use std::io::Read;
use std::thread::sleep;
use std::time::Duration;

fn main() {
    let reader = oneio::get_reader("http://josephine.sobornost.net/josephine.sobornost.net/rpkidata/2023/08/04/rpki-20230804T000430Z.tgz").unwrap();
    let mut ar = tar::Archive::new(reader);
    println!("processing rpkiviews tar file at http://josephine.sobornost.net/josephine.sobornost.net/rpkidata/2023/08/04/rpki-20230804T000430Z.tgz");
    println!("searching for any files in tar that ends with .csv");
    for entry in ar.entries().unwrap() {
        let mut entry = entry.unwrap();
        let path = entry.path().unwrap().to_string_lossy().to_string();
        if path.ends_with("csv") {
            println!("found file {}", &path);
            println!("reading content now... (sleep for 3 seconds)");
            sleep(Duration::from_secs(3));
            let mut content: String = String::default();
            entry.read_to_string(&mut content).unwrap();
            for line in content.lines() {
                println!("{}", line);
            }
        }
    }
}
