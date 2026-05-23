//! `/incidents/*` handlers.

use axum::extract::{Path, State};
use axum::Json;

use crate::api::dto::{
    parse_incident_id, EvidenceObservationDto, IncidentDetailDto, IncidentEvidenceDto,
    IncidentListDto, IncidentSummaryDto,
};
use crate::api::error::ApiError;
use crate::api::ApiDeps;

/// `GET /incidents/open` — every incident with `status != Resolved`,
/// newest first. No pagination in V0.
pub async fn list_open(State(deps): State<ApiDeps>) -> Result<Json<IncidentListDto>, ApiError> {
    let mut incidents = deps.incident_repo.load_open().await?;
    incidents.sort_by_key(|i| std::cmp::Reverse(i.opened_at));
    let summaries: Vec<IncidentSummaryDto> =
        incidents.iter().map(IncidentSummaryDto::from).collect();
    Ok(Json(IncidentListDto {
        count: summaries.len(),
        incidents: summaries,
    }))
}

/// `GET /incidents/:id` — full detail or 404.
pub async fn detail(
    State(deps): State<ApiDeps>,
    Path(id): Path<String>,
) -> Result<Json<IncidentDetailDto>, ApiError> {
    let parsed = parse_incident_id(&id)
        .map_err(|e| ApiError::BadRequest(format!("invalid incident id: {e}")))?;
    let incident = deps
        .incident_repo
        .get(&parsed)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("incident {id} not found")))?;
    Ok(Json(IncidentDetailDto::from(&incident)))
}

/// `GET /incidents/:id/evidence` — dereferences the incident's
/// evidence ObservationIds to full observations. Observations swept
/// by retention are silently omitted from the array.
pub async fn evidence(
    State(deps): State<ApiDeps>,
    Path(id): Path<String>,
) -> Result<Json<IncidentEvidenceDto>, ApiError> {
    let parsed = parse_incident_id(&id)
        .map_err(|e| ApiError::BadRequest(format!("invalid incident id: {e}")))?;
    let incident = deps
        .incident_repo
        .get(&parsed)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("incident {id} not found")))?;

    let mut observations: Vec<EvidenceObservationDto> = Vec::with_capacity(incident.evidence.len());
    for evidence_ref in &incident.evidence {
        match deps.observation_store.get_by_id(&evidence_ref.0).await? {
            Some(obs) => {
                let dto = EvidenceObservationDto::from_observation(&obs)?;
                observations.push(dto);
            }
            // Silently skip — retention may have swept the observation
            // out from under the incident. Operators see "evidence: []"
            // rather than a misleading 500.
            None => continue,
        }
    }

    Ok(Json(IncidentEvidenceDto {
        incident_id: incident.id.0,
        evidence: observations,
    }))
}
