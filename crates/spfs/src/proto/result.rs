// Copyright (c) Contributors to the SPK project.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/spkenv/spk

// This trait causes dead code warnings when the server feature is not enabled.
#[allow(dead_code)]
pub(crate) trait RpcResult: Sized {
    type Ok;
    type Result;
    fn error(err: crate::Error) -> Self;
    fn ok(data: Self::Ok) -> Self;
    fn to_result(self) -> crate::Result<Self::Ok>;
    fn from_result<T: Into<Self::Ok>>(result: crate::Result<T>) -> Self {
        match result {
            Err(err) => Self::error(err),
            Ok(ok) => Self::ok(ok.into()),
        }
    }
}

#[cfg(feature = "server")]
macro_rules! handle_error {
    ($result:expr) => {
        match $result {
            Err(err) => return Ok(tonic::Response::new(RpcResult::error(err))),
            Ok(data) => data,
        }
    };
}

#[cfg(feature = "server")]
pub(crate) use handle_error;

macro_rules! rpc_result {
    ($typename:ty, $result:ty) => {
        rpc_result!($typename, $result, super::Ok);
    };
    ($typename:ty, $result:ty, $ok_type:ty) => {
        impl RpcResult for $typename {
            type Ok = $ok_type;
            type Result = $result;
            fn error(err: crate::Error) -> Self {
                let result = Some(Self::Result::Error(err.into()));
                Self { result }
            }
            fn ok(data: Self::Ok) -> Self {
                let result = Some(Self::Result::Ok(data));
                Self { result }
            }
            fn to_result(self) -> crate::Result<Self::Ok> {
                match self.result {
                    Some(Self::Result::Error(err)) => Err(err.into()),
                    Some(Self::Result::Ok(data)) => Ok(data),
                    None => Err(crate::Error::String(format!(
                        "Unexpected empty result from the server"
                    ))),
                }
            }
        }
    };
}

use super::generated as gen;

rpc_result!(
    gen::LsTagsResponse,
    gen::ls_tags_response::Result,
    gen::ls_tags_response::EntryList
);
rpc_result!(
    gen::ResolveTagResponse,
    gen::resolve_tag_response::Result,
    gen::Tag
);
rpc_result!(
    gen::FindTagsResponse,
    gen::find_tags_response::Result,
    gen::find_tags_response::TagList
);
rpc_result!(
    gen::IterTagSpecsResponse,
    gen::iter_tag_specs_response::Result,
    gen::iter_tag_specs_response::TagSpecList
);
rpc_result!(
    gen::ReadTagResponse,
    gen::read_tag_response::Result,
    gen::read_tag_response::TagList
);
rpc_result!(gen::InsertTagResponse, gen::insert_tag_response::Result);
rpc_result!(
    gen::RemoveTagStreamResponse,
    gen::remove_tag_stream_response::Result
);
rpc_result!(gen::RemoveTagResponse, gen::remove_tag_response::Result);

rpc_result!(
    gen::ReadObjectResponse,
    gen::read_object_response::Result,
    gen::Object
);
rpc_result!(
    gen::FindDigestsResponse,
    gen::find_digests_response::Result,
    gen::Digest
);
rpc_result!(
    gen::IterDigestsResponse,
    gen::iter_digests_response::Result,
    gen::Digest
);
rpc_result!(
    gen::IterObjectsResponse,
    gen::iter_objects_response::Result,
    gen::Object
);
rpc_result!(
    gen::WalkObjectsResponse,
    gen::walk_objects_response::Result,
    gen::walk_objects_response::WalkObjectsItem
);
rpc_result!(gen::WriteObjectResponse, gen::write_object_response::Result);
rpc_result!(
    gen::RemoveObjectResponse,
    gen::remove_object_response::Result
);
rpc_result!(
    gen::RemoveObjectIfOlderThanResponse,
    gen::remove_object_if_older_than_response::Result,
    bool
);

rpc_result!(
    gen::WritePayloadResponse,
    gen::write_payload_response::Result,
    gen::write_payload_response::UploadOption
);
rpc_result!(
    gen::OpenPayloadResponse,
    gen::open_payload_response::Result,
    gen::open_payload_response::DownloadOption
);
rpc_result!(
    gen::RemovePayloadResponse,
    gen::remove_payload_response::Result
);
rpc_result!(
    gen::write_payload_response::UploadResponse,
    gen::write_payload_response::upload_response::Result,
    gen::write_payload_response::upload_response::UploadResult
);
