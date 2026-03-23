use std::io::Result;

fn main() -> Result<()> {
    #[cfg(windows)]
    {
        use std::path::Path;
        let icon_path = Path::new("assets/icon.ico");
        let logo_path = Path::new("assets/logo.png");
        
        // Generate icon if it doesn't exist (or is empty) and logo exists
        if (!icon_path.exists() || std::fs::metadata(icon_path).map(|m| m.len()).unwrap_or(0) == 0) 
            && logo_path.exists() {
            println!("cargo:warning=Generating icon from logo.png...");
            if let Ok(img) = image::open(logo_path) {
                // Resize to 256x256 for a high-quality icon that isn't massive
                let resized = img.resize(256, 256, image::imageops::FilterType::Lanczos3);
                if let Err(e) = resized.save_with_format(icon_path, image::ImageFormat::Ico) {
                    println!("cargo:warning=Failed to save icon.ico: {e}");
                } else {
                    println!("cargo:warning=Successfully generated icon.ico");
                }
            } else {
                println!("cargo:warning=Failed to open assets/logo.png");
            }
        }

        if icon_path.exists() && std::fs::metadata(icon_path).map(|m| m.len()).unwrap_or(0) > 0 {
            let mut res = winres::WindowsResource::new();
            res.set_icon(icon_path.to_str().unwrap());
            res.compile()?;
        } else {
            println!("cargo:warning=No valid icon found at assets/icon.ico - skipping embedding");
        }
    }
    
    Ok(())
}
