TEST (name "rpt_edge_case_model_keys__lists_one_key_per_model");

WITH __ref__int_edge_ratio_running_totals AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('ratio_runnin-1') AS v(check_id)
),

__ref__int_edge_ratio_averages AS (
  SELECT
    v.group_code::VARCHAR AS group_code
  FROM VALUES
    ('ratio_averag-1') AS v(group_code)
),

__ref__int_edge_callback_recent_offsets AS (
  SELECT
    v.callback_id::VARCHAR AS callback_id
  FROM VALUES
    ('callback_rec-1') AS v(callback_id)
),

__ref__int_edge_callback_epoch_reads AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('callback_epo-1') AS v(check_id)
),

__ref__int_edge_callback_zone_words AS (
  SELECT
    v.callback_id::VARCHAR AS callback_id
  FROM VALUES
    ('callback_zon-1') AS v(callback_id)
),

__ref__int_edge_callback_text_dates AS (
  SELECT
    v.callback_id::VARCHAR AS callback_id
  FROM VALUES
    ('callback_tex-1') AS v(callback_id)
),

__ref__int_edge_key_rank_offsets AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('key_rank_off-1') AS v(check_id)
),

__ref__int_edge_key_number_reads AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('key_number_r-1') AS v(check_id)
),

__ref__int_edge_callback_epoch_parts AS (
  SELECT
    v.callback_id::VARCHAR AS callback_id
  FROM VALUES
    ('callback_epo-1') AS v(callback_id)
),

__ref__int_edge_check_pair_filters AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_pair_f-1') AS v(check_id)
),

__ref__int_edge_check_wide_sums AS (
  SELECT
    v.key_row_id::VARCHAR AS key_row_id
  FROM VALUES
    ('check_wide_s-1') AS v(key_row_id)
),

__ref__int_edge_ratio_variant_unions AS (
  SELECT
    v.value_row_id::VARCHAR AS value_row_id
  FROM VALUES
    ('ratio_varian-1') AS v(value_row_id)
),

__ref__int_edge_check_note_unions AS (
  SELECT
    v.note_row_id::VARCHAR AS note_row_id
  FROM VALUES
    ('check_note_u-1') AS v(note_row_id)
),

__ref__int_edge_ratio_array_bounds AS (
  SELECT
    v.group_code::VARCHAR AS group_code
  FROM VALUES
    ('ratio_array_-1') AS v(group_code)
),

__ref__int_edge_check_zoned_literals AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_zoned_-1') AS v(check_id)
),

__ref__int_edge_callback_day_starts AS (
  SELECT
    v.callback_id::VARCHAR AS callback_id
  FROM VALUES
    ('callback_day-1') AS v(callback_id)
),

__ref__int_edge_callback_clock_times AS (
  SELECT
    v.callback_id::VARCHAR AS callback_id
  FROM VALUES
    ('callback_clo-1') AS v(callback_id)
),

__ref__int_edge_check_key_rewrites AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_key_re-1') AS v(check_id)
),

__ref__int_edge_variant_decimal_reads AS (
  SELECT
    v.value_id::VARCHAR AS value_id
  FROM VALUES
    ('variant_deci-1') AS v(value_id)
),

__ref__int_edge_ratio_safe_casts AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('ratio_safe_c-1') AS v(check_id)
),

__ref__int_edge_callback_safe_dates AS (
  SELECT
    v.callback_id::VARCHAR AS callback_id
  FROM VALUES
    ('callback_saf-1') AS v(callback_id)
),

__ref__int_edge_ratio_percent_texts AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('ratio_percen-1') AS v(check_id)
),

__ref__int_edge_callback_row_numbers AS (
  SELECT
    v.callback_id::VARCHAR AS callback_id
  FROM VALUES
    ('callback_row-1') AS v(callback_id)
),

__ref__int_edge_check_xml_checks AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_xml_ch-1') AS v(check_id)
),

__ref__int_edge_variant_xml_elements AS (
  SELECT
    v.value_id::VARCHAR AS value_id
  FROM VALUES
    ('variant_xml_-1') AS v(value_id)
),

__ref__int_edge_callback_latest_dates AS (
  SELECT
    v.merchant_zone::VARCHAR AS merchant_zone
  FROM VALUES
    ('callback_lat-1') AS v(merchant_zone)
),

__ref__int_edge_check_group_key_lists AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_group_-1') AS v(check_id)
),

__ref__int_edge_ratio_array_casts AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('ratio_array_-1') AS v(check_id)
),

__ref__int_edge_ratio_text_replaces AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('ratio_text_r-1') AS v(check_id)
),

__ref__int_edge_ratio_object_reads AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('ratio_object-1') AS v(check_id)
),

__ref__int_edge_ratio_object_flags AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('ratio_object-1') AS v(check_id)
),

__ref__int_edge_ratio_identity_objects AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('ratio_identi-1') AS v(check_id)
),

__ref__int_edge_ratio_array_positions AS (
  SELECT
    v.group_code::VARCHAR AS group_code
  FROM VALUES
    ('ratio_array_-1') AS v(group_code)
),

__ref__int_edge_check_local_times AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_local_-1') AS v(check_id)
),

__ref__int_edge_ratio_ignore_case_reads AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('ratio_ignore-1') AS v(check_id)
),

__ref__int_edge_callback_zoned_dates AS (
  SELECT
    v.callback_id::VARCHAR AS callback_id
  FROM VALUES
    ('callback_zon-1') AS v(callback_id)
),

__ref__int_edge_ratio_object_inserts AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('ratio_object-1') AS v(check_id)
),

__ref__int_edge_callback_date_lists AS (
  SELECT
    v.merchant_zone::VARCHAR AS merchant_zone
  FROM VALUES
    ('callback_dat-1') AS v(merchant_zone)
),

__ref__int_edge_check_bracket_strips AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_bracke-1') AS v(check_id)
),

__ref__int_edge_ratio_path_checks AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('ratio_path_c-1') AS v(check_id)
),

__ref__int_edge_ratio_fixed_scales AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('ratio_fixed_-1') AS v(check_id)
),

__ref__int_edge_check_formatted_dates AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_format-1') AS v(check_id)
),

__ref__int_edge_callback_last_day AS (
  SELECT
    v.callback_id::VARCHAR AS callback_id
  FROM VALUES
    ('callback_las-1') AS v(callback_id)
),

__ref__int_edge_check_month_starts AS (
  SELECT
    v.period_id::VARCHAR AS period_id
  FROM VALUES
    ('check_month_-1') AS v(period_id)
),

__ref__int_edge_variant_expected_values AS (
  SELECT
    v.value_id::VARCHAR AS value_id
  FROM VALUES
    ('variant_expe-1') AS v(value_id)
),

__ref__int_edge_check_reduced_keys AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_reduce-1') AS v(check_id)
),

__ref__int_edge_ratio_hashes AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('ratio_hashes-1') AS v(check_id)
),

__ref__int_edge_ratio_object_differences AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('ratio_object-1') AS v(check_id)
),

__ref__int_edge_check_group_flags AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_group_-1') AS v(check_id)
),

__ref__int_edge_check_label_lookups AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_label_-1') AS v(check_id)
),

__ref__int_edge_check_distinct_tuples AS (
  SELECT
    v.group_code::VARCHAR AS group_code
  FROM VALUES
    ('check_distin-1') AS v(group_code)
),

__ref__int_edge_callback_start_fallbacks AS (
  SELECT
    v.callback_id::VARCHAR AS callback_id
  FROM VALUES
    ('callback_sta-1') AS v(callback_id)
),

__ref__int_edge_amount_percent_texts AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('amount_perce-1') AS v(check_id)
),

__ref__int_edge_check_minute_times AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_minute-1') AS v(check_id)
),

__ref__int_edge_check_number_labels AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_number-1') AS v(check_id)
),

__ref__int_edge_check_decoded_keys AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_decode-1') AS v(check_id)
),

__ref__int_edge_callback_london_times AS (
  SELECT
    v.callback_id::VARCHAR AS callback_id
  FROM VALUES
    ('callback_lon-1') AS v(callback_id)
),

__ref__int_edge_check_pattern_matches AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_patter-1') AS v(check_id)
),

__ref__int_edge_check_any_amounts AS (
  SELECT
    v.group_code::VARCHAR AS group_code
  FROM VALUES
    ('check_any_am-1') AS v(group_code)
),

__ref__int_edge_check_sparse_arrays AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_sparse-1') AS v(check_id)
),

__ref__int_edge_check_last_known_keys AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_last_k-1') AS v(check_id)
),

__ref__int_edge_callback_day_documents AS (
  SELECT
    v.merchant_zone::VARCHAR AS merchant_zone
  FROM VALUES
    ('callback_day-1') AS v(merchant_zone)
),

__ref__int_edge_check_key_list_edits AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_key_li-1') AS v(check_id)
),

__ref__int_edge_ratio_group_objects AS (
  SELECT
    v.group_code::VARCHAR AS group_code
  FROM VALUES
    ('ratio_group_-1') AS v(group_code)
),

__ref__int_edge_ratio_text_forms AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('ratio_text_f-1') AS v(check_id)
),

__ref__int_edge_check_variant_branches AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_varian-1') AS v(check_id)
),

__ref__int_edge_ledger_quantity_steps AS (
  SELECT
    v.order_ref::VARCHAR AS order_ref
  FROM VALUES
    ('ledger_quant-1') AS v(order_ref)
),

__ref__int_edge_check_generated_rows AS (
  SELECT
    v.summary_id::VARCHAR AS summary_id
  FROM VALUES
    ('check_genera-1') AS v(summary_id)
),

__ref__int_edge_check_unnamed_labels AS (
  SELECT
    v.check_id::VARCHAR AS check_id
  FROM VALUES
    ('check_unname-1') AS v(check_id)
),

__expected__rpt_edge_case_model_keys AS (
  SELECT
    NULL AS model_key_id,
    NULL AS model_name,
    NULL AS model_key
)
