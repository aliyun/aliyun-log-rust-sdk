use super::*;
use crate::ResponseResult;
use getset::Getters;
use serde::Deserialize;

impl crate::client::Client {
    /// List all shards of a logstore.
    ///
    /// This method retrieves information about all shards in the specified logstore,
    /// including their IDs, status, key ranges, and creation times. Shards are the
    /// basic read and write units in Aliyun Log Service and are used to partition
    /// data for parallel processing.
    ///
    /// # Arguments
    ///
    /// * `project` - The name of the project containing the logstore
    /// * `logstore` - The name of the logstore to list shards for
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// # async fn example(client: aliyun_log_sdk::Client) -> Result<(), aliyun_log_sdk::Error> {
    /// // List all shards in the specified logstore
    /// let resp = client.list_shards("my-project", "my-logstore").send().await?;
    /// println!("Found {} shards", resp.get_body().shards().len());
    ///
    /// for shard in resp.get_body().shards() {
    ///     println!("Shard ID: {}, Status: {}", shard.shard_id(), shard.status());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn list_shards(
        &self,
        project: impl AsRef<str>,
        logstore: impl AsRef<str>,
    ) -> ListShardsRequestBuilder {
        ListShardsRequestBuilder {
            project: project.as_ref().to_string(),
            path: format!("/logstores/{}/shards", logstore.as_ref()),
            handle: self.handle.clone(),
        }
    }
}

pub struct ListShardsRequestBuilder {
    handle: HandleRef,
    project: String,
    path: String,
}

impl ListShardsRequestBuilder {
    #[must_use = "the result future must be awaited"]
    pub fn send(self) -> ResponseResultBoxFuture<ListShardsResponse> {
        Box::pin(async move {
            let (handle, request) = self.build()?;
            handle.send(request).await
        })
    }

    fn build(self) -> BuildResult<ListShardsRequest> {
        Ok((
            self.handle,
            ListShardsRequest {
                project: self.project,
                path: self.path,
            },
        ))
    }
}

#[derive(Debug, Getters, Default)]
pub struct ListShardsResponse {
    #[getset(get = "pub")]
    shards: Vec<list_shards_models::Shard>,
}

pub mod list_shards_models {
    use super::*;

    #[derive(Debug, Getters, Default, Deserialize)]
    #[getset(get = "pub")]
    pub struct Shard {
        #[serde(rename = "shardID")]
        shard_id: i32,

        /// The status of the shard. Possible values are `readwrite` and `readonly`.
        status: String,

        #[serde(rename = "inclusiveBeginKey")]
        inclusive_begin_key: String,

        #[serde(rename = "exclusiveEndKey")]
        exclusive_end_key: String,

        #[serde(rename = "createTime")]
        create_time: i64,
    }
}

impl FromHttpResponse for ListShardsResponse {
    fn try_from(body: bytes::Bytes, http_headers: &http::HeaderMap) -> ResponseResult<Self> {
        let shards: Vec<list_shards_models::Shard> =
            parse_json_response(body.as_ref(), http_headers)?;
        Ok(ListShardsResponse { shards })
    }
}

struct ListShardsRequest {
    project: String,
    path: String,
}

impl Request for ListShardsRequest {
    const HTTP_METHOD: http::Method = http::Method::GET;
    type ResponseBody = ListShardsResponse;
    fn project(&self) -> Option<&str> {
        Some(self.project.as_str())
    }
    fn path(&self) -> &str {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use crate::FromConfig;
    use lazy_static::lazy_static;
    lazy_static! {
        static ref TEST_CLIENT: crate::Client = {
            crate::client::Client::from_config(
                crate::client::Config::builder()
                    .access_key(
                        &crate::tests::TEST_ENV.access_key_id,
                        &crate::tests::TEST_ENV.access_key_secret,
                    )
                    .endpoint(&crate::tests::TEST_ENV.endpoint)
                    .build()
                    .unwrap(),
            )
            .unwrap()
        };
    }

    fn init() {
        let _ = env_logger::builder()
            .is_test(true)
            .filter_level(log::LevelFilter::Debug)
            .try_init();
    }

    #[tokio::test]
    async fn test() {
        init();
        let project = &crate::tests::TEST_ENV.project;
        let logstore = &crate::tests::TEST_ENV.logstore;
        let resp = TEST_CLIENT
            .list_shards(project, logstore)
            .send()
            .await
            .unwrap();
        println!("{:?}", resp.get_body().shards());
    }
}
