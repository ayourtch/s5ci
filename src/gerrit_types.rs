#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GerritQueryError {
    pub r#type: String,
    pub message: String,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GerritQueryStats {
    pub r#type: String,
    pub rowCount: u32,
    pub runTimeMilliseconds: u32,
    pub moreChanges: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GerritOwner {
    pub name: Option<String>,
    pub email: Option<String>,
    pub username: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GerritComment {
    pub timestamp: i64,
    pub reviewer: GerritOwner,
    pub message: String,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GerritApproval {
    pub r#type: String,
    pub description: Option<String>,
    pub value: String,
    pub grantedOn: i64,
    pub by: GerritOwner,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GerritFileChange {
    pub file: String,
    pub r#type: String,
    pub insertions: i32,
    pub deletions: i32,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GerritPatchSet {
    pub number: u32,
    pub revision: String,
    pub parents: Vec<String>,
    pub r#ref: String,
    pub uploader: GerritOwner,
    pub createdOn: i64,
    pub author: GerritOwner,
    pub kind: String,
    pub approvals: Option<Vec<GerritApproval>>,
    pub files: Option<Vec<GerritFileChange>>,
    pub sizeInsertions: Option<i32>,
    pub sizeDeletions: Option<i32>,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GerritDependentPatchSet {
    pub id: String,
    pub number: i32,
    pub revision: String,
    pub r#ref: String,
    pub isCurrentPatchSet: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GerritLabel {
    pub label: String,
    pub status: String,
    pub by: Option<GerritOwner>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GerritSubmitRecords {
    pub status: String,
    pub labels: Vec<GerritLabel>,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GerritChangeSet {
    pub project: Option<String>,
    pub branch: Option<String>,
    pub id: Option<String>,
    pub number: Option<u32>,
    pub subject: Option<String>,
    pub owner: Option<GerritOwner>,
    pub url: Option<String>,
    pub commitMessage: Option<String>,
    pub createdOn: Option<i64>,
    pub lastUpdated: Option<i64>,
    pub open: Option<bool>,
    pub status: Option<String>,
    pub comments: Option<Vec<GerritComment>>,
    pub patchSets: Option<Vec<GerritPatchSet>>,
    pub submitRecords: Option<Vec<GerritSubmitRecords>>,
    pub allReviewers: Option<Vec<GerritOwner>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum GerritVoteAction {
    success,
    failure,
    clear,
}

impl std::str::FromStr for GerritVoteAction {
    type Err = ();
    fn from_str(s: &str) -> Result<GerritVoteAction, ()> {
        match s {
            "success" => Ok(GerritVoteAction::success),
            "failure" => Ok(GerritVoteAction::failure),
            "clear" => Ok(GerritVoteAction::clear),
            _ => Err(()),
        }
    }
}
