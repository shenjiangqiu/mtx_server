use bytes::BufMut;
use futures::TryStreamExt;
use std::convert::Infallible;
use warp::{
    http::StatusCode,
    multipart::{FormData, Part},
    path, Filter, Rejection, Reply,
};
#[tokio::main]
async fn main() {
    let file_list = path!("filelist").map(|| {
        // build a list of files in the current directory as json format
        let mut files = Vec::new();
        for entry in std::fs::read_dir("./mtx").unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let metadata = path.metadata().unwrap();
            if metadata.is_file() {
                files.push(path.file_name().unwrap().to_str().unwrap().to_string());
            }
        }
        serde_json::to_string_pretty(&files).unwrap()
    });
    let update_git = warp::path("updategit").map(|| {
        let mut cmd = std::process::Command::new("git");
        cmd.arg("pull");
        let output = cmd.output().unwrap();
        warp::reply::with_header(output.stdout, "Content-Type", "text/plain")
    });
    let files = warp::path("files")
        .and(warp::fs::dir("mtx/"))
        .map(|x| warp::reply::with_header(x, "Content-Type", "text/plain"));
    let upload_route = warp::path("upload")
        .and(warp::post())
        .and(warp::multipart::form().max_length(5_000_000))
        .and_then(upload);

    let routes = file_list
        .or(files)
        .or(upload_route)
        .or(update_git)
        .recover(handle_rejection);
    let routes = routes.map(|x| warp::reply::with_header(x, "Access-Control-Allow-Origin", "*"));

    println!("running on http://0.0.0.0:3030");
    warp::serve(routes).run(([0, 0, 0, 0], 3030)).await;
}

async fn upload(form: FormData) -> Result<impl Reply, Rejection> {
    let parts: Vec<Part> = form.try_collect().await.map_err(|e| {
        eprintln!("form error: {}", e);
        warp::reject::reject()
    })?;

    for p in parts {
        if p.name() == "file" {
            let file_name = p.filename().unwrap().to_string();
            let value = p
                .stream()
                .try_fold(Vec::new(), |mut vec, data| {
                    vec.put(data);
                    async move { Ok(vec) }
                })
                .await
                .map_err(|e| {
                    eprintln!("reading file error: {}", e);
                    warp::reject::reject()
                })?;

            tokio::fs::write(format!("mtx/{}", file_name), value)
                .await
                .map_err(|e| {
                    eprint!("error writing file: {}", e);
                    warp::reject::reject()
                })?;
            println!("created file: {}", file_name);
        }
    }

    Ok("success")
}

async fn handle_rejection(err: Rejection) -> std::result::Result<impl Reply, Infallible> {
    let (code, message) = if err.is_not_found() {
        (StatusCode::NOT_FOUND, "Not Found".to_string())
    } else if err.find::<warp::reject::PayloadTooLarge>().is_some() {
        (StatusCode::BAD_REQUEST, "Payload too large".to_string())
    } else {
        eprintln!("unhandled error: {:?}", err);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal Server Error".to_string(),
        )
    };

    Ok(warp::reply::with_status(message, code))
}
