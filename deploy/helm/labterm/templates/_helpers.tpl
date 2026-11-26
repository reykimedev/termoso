{{- define "labterm.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "labterm.fullname" -}}
{{- if .Values.fullnameOverride -}}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" -}}
{{- else -}}
{{- printf "%s-%s" .Release.Name (include "labterm.name" .) | trunc 63 | trimSuffix "-" -}}
{{- end -}}
{{- end -}}

{{- define "labterm.labels" -}}
helm.sh/chart: {{ printf "%s-%s" .Chart.Name .Chart.Version }}
app.kubernetes.io/name: {{ include "labterm.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end -}}

{{- define "labterm.selectorLabels" -}}
app.kubernetes.io/name: {{ include "labterm.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end -}}

{{- define "labterm.image" -}}
{{- printf "%s:%s" .Values.image.repository (default .Chart.AppVersion .Values.image.tag) -}}
{{- end -}}

{{- define "labterm.secretName" -}}
{{- default (printf "%s-env" (include "labterm.fullname" .)) .Values.existingSecret -}}
{{- end -}}

{{- define "labterm.databaseUrl" -}}
{{- if .Values.bundled.postgres.enabled -}}
{{- printf "postgres://labterm:%s@%s-postgres:5432/labterm" .Values.bundled.postgres.password (include "labterm.fullname" .) -}}
{{- else -}}
{{- .Values.database.url -}}
{{- end -}}
{{- end -}}

{{- define "labterm.redisUrl" -}}
{{- if .Values.bundled.redis.enabled -}}
{{- printf "redis://%s-redis:6379" (include "labterm.fullname" .) -}}
{{- else -}}
{{- .Values.redis.url -}}
{{- end -}}
{{- end -}}
