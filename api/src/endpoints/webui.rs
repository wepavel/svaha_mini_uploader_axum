use axum::extract::Path;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use my_core::config::CONFIG;
use crate::exceptions::{ErrorCode, BadResponseObject, HtmlResponse};
use axum::response::Html as AxumHtml;

const TAG: &str = "WebUI";
pub fn get_router() -> OpenApiRouter {
    OpenApiRouter::new().routes(routes!(upload_ui))
}

#[utoipa::path(
    get,
    tag=TAG,
    path = "/{session_id}/{track_id}/{file_type}",
    params(
        ("session_id" = String, Path, description = "Session id"),
        ("track_id" = String, Path, description = "Id of track"),
        ("file_type" = String, Path, description = "Type: **vocal** or **instrumental**"),
    ),
    responses(
        (status = 200, description = "Number returned successfully", body = String, content_type = "text/html"),
        (status = 400, description = "Bad request", body = BadResponseObject, example = json!(BadResponseObject::default_400())),
        (status = 500, description = "Internal server error", body = BadResponseObject, example = json!(BadResponseObject::default_500())),
    )
)]
pub async fn upload_ui(Path((session_id, track_id, file_type)): Path<(String, String, String)>) -> HtmlResponse {
    if session_id == "ses" {
        return HtmlResponse::from(ErrorCode::AuthorizeError.details()
            .with_detail("reason", "You have already taken access to this endpoint."));
    }

    tracing::info!("Hello from tracing!");
    let html_code = file_upload_html(&session_id, &track_id, &file_type);

    HtmlResponse::Ok(html_code)
}




fn file_upload_html(session_id: &str, track_id: &str, file_type: &str) -> String {
    let p0 = r#"
<!DOCTYPE html>
<head>
    <meta charset="UTF-8">
    <style>
        * {
            font-size: 12px !important;
            color: #0f0 ;
            font-family: Andale Mono !important;
            background-color: #000;
        }

        .progress-wrapper {
            width:100%;
        }

        .progress-wrapper .progress {
            background-color: #0f0;
            width:0%;
            padding:5px 0px 5px 0px;
            color: black;
        }

        .file-upload-button {
            padding: 5px 10px;
            border: 1px solid #0f0;
            cursor: pointer;
            display: inline-block;
            margin-top: 30px;
        }

        .file-upload input[type="file"] {
            display: none;
        }

         input {
             font-size: 12px !important;
             color: #0f0 ;
             font-family: Andale Mono !important;
             background-color: #000;
                }

    </style>
    "#;

    let p1 = r#"
    <script>
        function postFile() {
            var formdata = new FormData();

            formdata.append('file1', document.getElementById('file1').files[0]);

            var request = new XMLHttpRequest();

            request.upload.addEventListener('progress', function (e) {
                var file1Size = document.getElementById('file1').files[0].size;
                console.log(file1Size);

                if (e.loaded <= file1Size) {
                    var percent = Math.round(e.loaded / file1Size * 100);
                    document.getElementById('progress-bar-file1').style.width = percent + '%';
                    document.getElementById('progress-bar-file1').innerHTML = percent + '%';
                }

                if(e.loaded == e.total){
                    document.getElementById('progress-bar-file1').style.width = '100%';
                    document.getElementById('progress-bar-file1').innerHTML = '100%';
                }
            });
    "#;

    let p2 = format!(r#"
            request.open('post', 'http://127.0.0.1:8023/api/v1/upload_s3/{}/s/s');
            // request.timeout = 45000;
            request.send(formdata);
    "#, session_id);

    let p3 = r#"
        }
    </script>
</head>
<form id="form1">

    <div class="file-upload">
        <label for="file1" class="file-upload-button">Choose file</label>
        <input id="file1" type="file" />

    </div>
    <div class="progress-wrapper">
        <div id="progress-bar-file1" class="progress"></div>
    </div>
    <button type="button" onclick="postFile()">Upload File</button>
</form>
</html>
    "#;

    format!("{}{}{}{}", p0, p1, p2, p3)
}




fn file_upload_multiple_html(session_id: &str, track_id: &str,) -> String {
    let p0 = r#"
<!DOCTYPE html>
<head>
    <meta charset="UTF-8">
    <style>
        * {
            font-size: 12px !important;
            color: #0f0 ;
            font-family: Andale Mono !important;
            background-color: #000;
        }

        .progress-wrapper {
            width:100%;
            margin-bottom: 10px;
        }

        .progress-wrapper .progress {
            background-color: #0f0;
            width:0%;
            padding:5px 0px 5px 0px;
            color: black;
        }

        .file-upload-button {
            padding: 5px 10px;
            border: 1px solid #0f0;
            cursor: pointer;
            display: inline-block;
            margin-top: 10px;
        }

        .file-upload input[type="file"] {
            display: none;
        }

        input {
            font-size: 12px !important;
            color: #0f0 ;
            font-family: Andale Mono !important;
            background-color: #000;
        }

        .file-path {
            margin-left: 10px;
        }
    </style>
    "#;

    let p1 = r#"
    <script>
        function updateFilePath(inputId, pathId) {
            var filePath = document.getElementById(inputId).value;
            document.getElementById(pathId).textContent = filePath.split('\\').pop();
        }

        function postFiles() {
            var formdata = new FormData();
            var vocal = document.getElementById('vocal').files[0];
            var instrumental = document.getElementById('instrumental').files[0];

            if (!vocal || !instrumental) {
                alert('Please select both files');
                return;
            }

            formdata.append('vocal', vocal);
            formdata.append('instrumental', instrumental);

            var request = new XMLHttpRequest();

            function updateProgress(loaded, total, fileType) {
                var percent = Math.round((loaded / total) * 100);
                document.getElementById('progress-bar-' + fileType).style.width = percent + '%';
                document.getElementById('progress-bar-' + fileType).innerHTML = percent + '%';
            }

            var totalSize = vocal.size + instrumental.size;
            var vocalUploaded = false;

            request.upload.addEventListener('progress', function (e) {
                var uploadedSize = e.loaded;

                if (!vocalUploaded && uploadedSize <= vocal.size) {
                    // Загрузка вокала
                    updateProgress(uploadedSize, vocal.size, 'vocal');
                } else if (!vocalUploaded && uploadedSize > vocal.size) {
                    // Вокал загружен, начинаем инструментал
                    updateProgress(vocal.size, vocal.size, 'vocal');
                    vocalUploaded = true;
                    updateProgress(uploadedSize - vocal.size, instrumental.size, 'instrumental');
                } else {
                    // Продолжаем загрузку инструментала
                    updateProgress(uploadedSize - vocal.size, instrumental.size, 'instrumental');
                }
            });

            request.onload = function() {
                if (request.status === 200) {
                    console.log('Files uploaded successfully');
                    // Убедимся, что оба прогресс-бара показывают 100%
                    updateProgress(vocal.size, vocal.size, 'vocal');
                    updateProgress(instrumental.size, instrumental.size, 'instrumental');
                } else {
                    console.error('An error occurred during the upload');
                }
            };
    "#;

    let p2 = format!(r#"
            request.open('post', '{}/api/v1/upload_s3_multiple/{}/{}');
            request.send(formdata);
    "#, CONFIG.upload_public_domain, session_id, track_id);

    let p3 = r#"
        }
    </script>
</head>
<form id="form1">
    <div class="file-upload">
        <label for="vocal" class="file-upload-button">Choose vocal file</label>
        <input id="vocal" type="file" onchange="updateFilePath('vocal', 'vocal-path')" />
        <span id="vocal-path" class="file-path"></span>
    </div>
    <div class="progress-wrapper">
        <div id="progress-bar-vocal" class="progress"></div>
    </div>

    <div class="file-upload">
        <label for="instrumental" class="file-upload-button">Choose instrumental file</label>
        <input id="instrumental" type="file" onchange="updateFilePath('instrumental', 'instrumental-path')" />
        <span id="instrumental-path" class="file-path"></span>
    </div>
    <div class="progress-wrapper">
        <div id="progress-bar-instrumental" class="progress"></div>
    </div>

    <button type="button" onclick="postFiles()">Upload Files</button>
</form>
</html>
    "#;

    format!("{}{}{}{}", p0, p1, p2, p3)
}

fn file_upload_multiple_filesystem_html(session_id: &str, track_id: &str,) -> String {
    let p0 = r#"
<!DOCTYPE html>
<head>
    <meta charset="UTF-8">
    <style>
        * {
            font-size: 12px !important;
            color: #0f0 ;
            font-family: Andale Mono !important;
            background-color: #000;
        }

        .progress-wrapper {
            width:100%;
            margin-bottom: 10px;
        }

        .progress-wrapper .progress {
            background-color: #0f0;
            width:0%;
            padding:5px 0px 5px 0px;
            color: black;
        }

        .file-upload-button {
            padding: 5px 10px;
            border: 1px solid #0f0;
            cursor: pointer;
            display: inline-block;
            margin-top: 10px;
        }

        .file-upload input[type="file"] {
            display: none;
        }

        input {
            font-size: 12px !important;
            color: #0f0 ;
            font-family: Andale Mono !important;
            background-color: #000;
        }

        .file-path {
            margin-left: 10px;
        }
    </style>
    "#;

    let p1 = r#"
    <script>
        function updateFilePath(inputId, pathId) {
            var filePath = document.getElementById(inputId).value;
            document.getElementById(pathId).textContent = filePath.split('\\').pop();
        }

        function postFiles() {
            var formdata = new FormData();
            var vocal = document.getElementById('vocal').files[0];
            var instrumental = document.getElementById('instrumental').files[0];

            if (!vocal || !instrumental) {
                alert('Please select both files');
                return;
            }

            formdata.append('vocal', vocal);
            formdata.append('instrumental', instrumental);

            var request = new XMLHttpRequest();

            function updateProgress(loaded, total, fileType) {
                var percent = Math.round((loaded / total) * 100);
                document.getElementById('progress-bar-' + fileType).style.width = percent + '%';
                document.getElementById('progress-bar-' + fileType).innerHTML = percent + '%';
            }

            var totalSize = vocal.size + instrumental.size;
            var vocalUploaded = false;

            request.upload.addEventListener('progress', function (e) {
                var uploadedSize = e.loaded;

                if (!vocalUploaded && uploadedSize <= vocal.size) {
                    // Загрузка вокала
                    updateProgress(uploadedSize, vocal.size, 'vocal');
                } else if (!vocalUploaded && uploadedSize > vocal.size) {
                    // Вокал загружен, начинаем инструментал
                    updateProgress(vocal.size, vocal.size, 'vocal');
                    vocalUploaded = true;
                    updateProgress(uploadedSize - vocal.size, instrumental.size, 'instrumental');
                } else {
                    // Продолжаем загрузку инструментала
                    updateProgress(uploadedSize - vocal.size, instrumental.size, 'instrumental');
                }
            });

            request.onload = function() {
                if (request.status === 200) {
                    console.log('Files uploaded successfully');
                    // Убедимся, что оба прогресс-бара показывают 100%
                    updateProgress(vocal.size, vocal.size, 'vocal');
                    updateProgress(instrumental.size, instrumental.size, 'instrumental');
                } else {
                    console.error('An error occurred during the upload');
                }
            };
    "#;

    let p2 = format!(r#"
            request.open('post', '{}/api/v1/upload_filesystem_multiple/{}/{}');
            request.send(formdata);
    "#, CONFIG.upload_public_domain, session_id, track_id);

    let p3 = r#"
        }
    </script>
</head>
<form id="form1">
    <div class="file-upload">
        <label for="vocal" class="file-upload-button">Choose vocal file</label>
        <input id="vocal" type="file" onchange="updateFilePath('vocal', 'vocal-path')" />
        <span id="vocal-path" class="file-path"></span>
    </div>
    <div class="progress-wrapper">
        <div id="progress-bar-vocal" class="progress"></div>
    </div>

    <div class="file-upload">
        <label for="instrumental" class="file-upload-button">Choose instrumental file</label>
        <input id="instrumental" type="file" onchange="updateFilePath('instrumental', 'instrumental-path')" />
        <span id="instrumental-path" class="file-path"></span>
    </div>
    <div class="progress-wrapper">
        <div id="progress-bar-instrumental" class="progress"></div>
    </div>

    <button type="button" onclick="postFiles()">Upload Files</button>
</form>
</html>
    "#;

    format!("{}{}{}{}", p0, p1, p2, p3)
}