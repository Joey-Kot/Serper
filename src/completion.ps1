Register-ArgumentCompleter -Native -CommandName 'serper' -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)

    $words = [System.Collections.Generic.List[string]]::new()
    $quote = $null
    foreach ($element in $commandAst.CommandElements) {
        if ($element.Extent.StartOffset -ge $cursorPosition) {
            break
        }
        if ($wordToComplete -ne '' -and $element.Extent.EndOffset -ge $cursorPosition) {
            if ($element -is [System.Management.Automation.Language.StringConstantExpressionAst] -and
                $element.StringConstantType -eq 'SingleQuoted') {
                $quote = "'"
                $words.Add($element.Value)
            } elseif ($element -is [System.Management.Automation.Language.StringConstantExpressionAst] -and
                      $element.StringConstantType -eq 'DoubleQuoted') {
                $quote = '"'
                $words.Add($element.Value)
            } else {
                $words.Add($wordToComplete)
            }
            break
        }
        if ($element -is [System.Management.Automation.Language.StringConstantExpressionAst]) {
            $words.Add($element.Value)
        } else {
            $words.Add($element.Extent.Text)
        }
    }
    if ($wordToComplete -eq '') {
        $words.Add('')
    }

    $previous = $env:SERPER_COMPLETE
    try {
        $env:SERPER_COMPLETE = 'powershell'
        $results = & serper -- @words
    } finally {
        if ($null -eq $previous) {
            Remove-Item Env:SERPER_COMPLETE -ErrorAction SilentlyContinue
        } else {
            $env:SERPER_COMPLETE = $previous
        }
    }
    foreach ($result in $results) {
        $parts = $result -split "`t", 2
        $description = if ($parts.Count -gt 1) { $parts[1] } else { $parts[0] }
        $candidate = if ($quote -eq "'") {
            "'" + $parts[0].Replace("'", "''") + "'"
        } elseif ($quote -eq '"') {
            '"' + $parts[0].Replace('`', '``').Replace('$', '`$').Replace('"', '`"') + '"'
        } else {
            $parts[0]
        }
        [System.Management.Automation.CompletionResult]::new(
            $candidate, $parts[0], 'ParameterValue', $description
        )
    }
}
