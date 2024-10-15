use std::{collections::BTreeMap, str::FromStr};

use pyo3::{exceptions::PyRuntimeError, prelude::*};
use reqwest::{
    header::{HeaderMap, HeaderName},
    RequestBuilder,
};

/// A Python module implemented in Rust.
#[pymodule]
fn pyodide_reqwest_demo(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(http_get, m)?)?;
    m.add_function(wrap_pyfunction!(http_get_async, m)?)?;
    Ok(())
}

/// Perform a HTTP GET
#[pyfunction]
#[pyo3(signature = (url, headers = None, body = None))]
fn http_get(
    url: String,
    headers: Option<BTreeMap<String, String>>,
    body: Option<Vec<u8>>,
    py: Python,
) -> PyResult<Py<ReqwestResponse>> {
    let builder = setup_request(headers, body, reqwest::Client::new().get(url));

    let reqwest_response = if false {
        // let runtime = tokio::runtime::Builder::new_current_thread()
        //     .build()
        //     .unwrap();
        // py.allow_threads(|| runtime.block_on(get_response(builder)))?

        panic!()
    } else {
        let ffi = PyModule::import_bound(py, "pyodide.ffi")?;
        let run_sync = ffi.getattr("run_sync")?;

        let fut = get_response(builder);
        let coroutine = pyo3_async_runtimes::tokio::future_into_py(py, fut)?;

        let py_rr = run_sync.call1((coroutine,))?;

        let reqwest_response: Bound<ReqwestResponse> = py_rr
            .downcast_into()
            .map_err(|_| PyRuntimeError::new_err("downcast failed"))?;

        reqwest_response.unbind()
    };

    Ok(reqwest_response)
}

#[pyfunction]
#[pyo3(signature = (url, headers = None, body = None))]
fn http_get_async(
    url: String,
    headers: Option<BTreeMap<String, String>>,
    body: Option<Vec<u8>>,
    py: Python,
) -> PyResult<PyObject> {
    println!("http get async");

    let builder = setup_request(headers, body, reqwest::Client::new().get(url));
    let response_future = get_response(builder);

    let coroutine = pyo3_async_runtimes::tokio::future_into_py(py, response_future)?;
    Ok(coroutine.unbind())
}

/// A reqwest HTTP response
#[pyclass]
struct ReqwestResponse {
    response: Option<reqwest::Response>,
}

#[pymethods]
impl ReqwestResponse {
    pub fn status(&self) -> PyResult<u16> {
        Ok(self.inner()?.status().into())
    }

    pub fn url(&self) -> PyResult<String> {
        Ok(self.inner()?.url().to_string())
    }

    pub fn headers(&self) -> PyResult<BTreeMap<String, String>> {
        let mut dict = BTreeMap::<String, String>::default();
        for (key, value) in self.inner()?.headers() {
            dict.insert(key.to_string(), value.to_str().unwrap().to_string());
        }

        Ok(dict)
    }

    pub fn text(&mut self, py: Python) -> PyResult<String> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let fut = self.take_inner()?.text();
        let text = py
            .allow_threads(|| runtime.block_on(fut))
            .map_err(|e| PyRuntimeError::new_err(format!("{e:?}")))?;

        Ok(text)
    }
}

impl ReqwestResponse {
    fn inner(&self) -> PyResult<&reqwest::Response> {
        self.response
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("response already consumed"))
    }

    fn take_inner(&mut self) -> PyResult<reqwest::Response> {
        self.response
            .take()
            .ok_or_else(|| PyRuntimeError::new_err("response already consumed"))
    }
}

fn setup_request(
    headers: Option<BTreeMap<String, String>>,
    body: Option<Vec<u8>>,
    mut builder: RequestBuilder,
) -> RequestBuilder {
    if let Some(headers) = headers {
        let mut header_map = HeaderMap::new();
        for (key, value) in headers {
            header_map.insert(
                HeaderName::from_str(&key).unwrap(),
                value.try_into().unwrap(),
            );
        }
        builder = builder.headers(header_map);
    }

    if let Some(body) = body {
        builder = builder.body(body);
    }

    builder
}

async fn get_response(builder: reqwest::RequestBuilder) -> PyResult<ReqwestResponse> {
    let response = builder
        .send()
        .await
        .map_err(|e| PyRuntimeError::new_err(format!("{e:?}")))?;

    Ok(ReqwestResponse {
        response: Some(response),
    })
}
