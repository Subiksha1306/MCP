pub mod sharepoint;
pub mod dataverse;
pub mod auth;

pub use sharepoint::SharePointConnector;
pub use dataverse::DataverseConnector;
pub use auth::get_azure_token;