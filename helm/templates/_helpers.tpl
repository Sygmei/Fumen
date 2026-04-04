{{- define "musescore-reader.name" -}}
musescore-reader
{{- end -}}

{{- define "musescore-reader.fullname" -}}
{{- printf "%s-%s" .Release.Name (include "musescore-reader.name" .) | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "musescore-reader.backend.fullname" -}}
{{- printf "%s-backend" (include "musescore-reader.fullname" .) | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "musescore-reader.frontend.fullname" -}}
{{- printf "%s-frontend" (include "musescore-reader.fullname" .) | trunc 63 | trimSuffix "-" -}}
{{- end -}}
