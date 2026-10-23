import { ex, type Section } from "./types";

export const pt: Section[] = [
  {
    id: "start",
    title: "Primeiros passos",
    html: `
<p>O Renome renomeia muitos arquivos de uma vez. O trabalho tem três etapas:</p>
<ol>
  <li><b>Adicione os arquivos</b>: arraste arquivos ou pastas para a janela, ou use <b>+ Adicionar</b>.</li>
  <li><b>Monte as regras</b>: em <b>+ Regra</b>, escolha o que fazer com os nomes (substituir texto, numerar, inserir uma data…). A coluna <b>Novo nome</b> mostra o resultado na hora, antes de qualquer alteração no disco.</li>
  <li><b>Renomeie</b>: clique em <b>Renomear</b> (ou <kbd>Ctrl</kbd>+<kbd>Enter</kbd>). Se algo sair errado, desfaça pelo <b>Histórico</b>.</li>
</ol>
<p>Nada é alterado no disco até você clicar em Renomear. Pode experimentar à vontade.</p>`,
  },
  {
    id: "files",
    title: "Lista de arquivos",
    html: `
<h4>Adicionar</h4>
<p>O botão <b>+ Adicionar</b> tem três opções:</p>
<ul>
  <li><b>Arquivos…</b>: escolha um ou vários arquivos.</li>
  <li><b>Pastas (com subpastas)…</b>: adiciona todos os arquivos de dentro das pastas, inclusive das subpastas.</li>
  <li><b>Pastas (renomear as próprias pastas)…</b>: adiciona as pastas em si, para mudar o nome delas. Elas aparecem com um ícone de pasta.</li>
</ul>
<p>Arrastar para a janela funciona como "Arquivos" e "Pastas (com subpastas)". Arquivos que já estão na lista são ignorados, e os recém-adicionados ficam selecionados.</p>
<p>Arquivos ocultos (que começam com ponto, como <code>.config</code>) ficam de fora ao adicionar pastas, a não ser que você ligue a opção correspondente.</p>
<p>Também dá para abrir o programa já com arquivos: <code>renome foto1.jpg foto2.jpg pasta/</code>.</p>

<h4>Filtrar</h4>
<p>O campo <b>Filtrar</b> mostra só os arquivos que combinam. <b>Só os arquivos visíveis são renomeados</b>, e a numeração conta só eles.</p>
<ul>
  <li>Termos separados por vírgula ou espaço: <code>jpg, férias</code> mostra os nomes que contêm "jpg" <i>ou</i> "férias".</li>
  <li><code>*</code> e <code>?</code> são curingas: <code>*.png</code> (termina em .png), <code>IMG_????.jpg</code> (quatro caracteres quaisquer).</li>
</ul>

<h4>Ordenar e organizar</h4>
<ul>
  <li>Clique no título de uma coluna para ordenar; clique de novo para inverter.</li>
  <li>A ordem padrão é por <b>Novo nome</b>: a lista se reorganiza enquanto você mexe nas regras.</li>
  <li>Selecione com clique, <kbd>Ctrl</kbd>+clique (vários) e <kbd>Shift</kbd>+clique (intervalo).</li>
  <li><b>⤒ ⤓</b> movem os selecionados para cima ou para baixo, útil para acertar a numeração. Não funcionam na ordem por Novo nome.</li>
  <li><b>Remover</b> (ou <kbd>Delete</kbd>) tira os selecionados da lista; <b>Limpar</b> esvazia a lista. Os arquivos no disco não são tocados.</li>
  <li><kbd>Ctrl</kbd>+<kbd>Z</kbd> desfaz a última alteração na lista.</li>
</ul>`,
  },
  {
    id: "rules",
    title: "Como as regras funcionam",
    html: `
<p>As regras são aplicadas <b>de cima para baixo</b>, cada uma sobre o resultado da anterior. A ordem importa: numerar e depois trocar maiúsculas não dá o mesmo resultado que o contrário.</p>
<ul>
  <li>O <b>checkbox</b> no título liga e desliga a regra sem apagá-la.</li>
  <li><b>↑ ↓</b> mudam a ordem; <b>✕</b> remove a regra; clicar no título recolhe o card.</li>
  <li>Uma regra com erro (expressão inválida, por exemplo) fica com borda vermelha e é ignorada até ser corrigida.</li>
</ul>
<h4>Aplicar em</h4>
<p>Várias regras perguntam onde agir:</p>
<ul>
  <li><b>Nome (sem extensão)</b>: o padrão. Em <code>foto.jpg</code>, age só em <code>foto</code>.</li>
  <li><b>Extensão</b>: só em <code>jpg</code>.</li>
  <li><b>Nome completo</b>: em <code>foto.jpg</code> inteiro.</li>
</ul>
<p>Pastas não têm extensão: em <code>Série.2020</code>, o <code>.2020</code> faz parte do nome.</p>
<h4>Salvar e abrir regras</h4>
<p>📂 abre e 💾 salva o conjunto de regras num arquivo <code>.json</code>, para reaproveitar depois. Por padrão, o programa também lembra as regras ao ser fechado.</p>
<h4>Posições</h4>
<p>Onde uma regra pede uma posição, a contagem é de caracteres visíveis a partir de 1. Letras acentuadas e emojis contam como um caractere.</p>`,
  },
  {
    id: "r-replace",
    title: "Substituir texto",
    sub: true,
    html: `
<p>Troca um texto por outro.</p>
${ex("foto_de_ferias.jpg", "foto de ferias.jpg")}
<ul>
  <li><b>Localizar</b> e <b>Substituir por</b>: deixe "Substituir por" vazio para apagar o texto.</li>
  <li><b>Diferenciar maiúsculas</b>: desmarcado, "Foto" também encontra "foto" e "FOTO".</li>
  <li><b>Só a 1ª ocorrência em cada nome</b>: troca só a primeira vez que o texto aparece <i>em cada nome</i>. Em <code>a_b_c</code>, trocar <code>_</code> por espaço dá <code>a b_c</code>. Se o texto só aparece uma vez, todos os nomes mudam do mesmo jeito.</li>
</ul>`,
  },
  {
    id: "r-regex",
    title: "Expressão regular",
    sub: true,
    html: `
<p>Para padrões que o "Substituir texto" não alcança. Os grupos entre parênteses podem ser reaproveitados na substituição com <code>$1</code>, <code>$2</code>…</p>
${ex("01 - Abertura.mp3", "Abertura (01).mp3")}
<p>Expressão <code>(\\d+) - (.*)</code>, substituir por <code>$2 ($1)</code>.</p>
<table>
  <tr><td><code>.</code></td><td>qualquer caractere</td></tr>
  <tr><td><code>\\d</code> <code>\\s</code> <code>\\w</code></td><td>dígito, espaço, letra/dígito/_</td></tr>
  <tr><td><code>*</code> <code>+</code> <code>?</code></td><td>zero ou mais, um ou mais, opcional</td></tr>
  <tr><td><code>{2}</code> <code>{1,3}</code></td><td>exatamente 2, de 1 a 3 vezes</td></tr>
  <tr><td><code>[abc]</code> <code>[^abc]</code></td><td>um desses caracteres, qualquer um menos esses</td></tr>
  <tr><td><code>^</code> <code>$</code></td><td>início e fim do texto</td></tr>
  <tr><td><code>( )</code></td><td>grupo, reaproveitado como <code>$1</code>, <code>$2</code>…</td></tr>
  <tr><td><code>\\.</code> <code>\\(</code></td><td>o próprio ponto, o próprio parêntese</td></tr>
</table>
<p class="tip">Se um grupo é seguido de letras ou dígitos, use chaves: <code>\${1}a</code>. Escrito <code>$1a</code>, o programa procura um grupo chamado "1a", que não existe.</p>
<p>Grupos com nome também funcionam: <code>(?P&lt;ano&gt;\\d{4})</code> e depois <code>\${ano}</code>.</p>`,
  },
  {
    id: "r-rearrange",
    title: "Reorganizar partes",
    sub: true,
    html: `
<p>Divide o nome num separador e monta de novo em outra ordem. Não precisa de expressões regulares.</p>
${ex("Série - Episódio - S01E02.mkv", "Série - S01E02 - Episódio.mkv")}
<ul>
  <li><b>Separador</b>: o texto que divide as partes (aqui, <code> - </code>). Espaços em volta de cada parte são removidos, e partes vazias são descartadas.</li>
  <li>O card mostra as partes do primeiro arquivo da lista, numeradas. <b>Clique numa parte</b> para acrescentá-la à nova ordem.</li>
  <li><b>Nova ordem</b>: números são as partes; todo o resto é mantido como texto. Para o exemplo: <code>1 - 3 - 2</code>.</li>
  <li>Partes que não aparecem na ordem são removidas. Ordem <code>2 (1)</code> em <code>Silva - Ana</code> dá <code>Ana (Silva)</code>.</li>
  <li>Se um nome tem menos partes do que a ordem pede, ele fica como está.</li>
</ul>`,
  },
  {
    id: "r-case",
    title: "Maiúsculas / minúsculas",
    sub: true,
    html: `
<ul>
  <li><b>minúsculas</b>: <code>Foto De Férias</code> → <code>foto de férias</code></li>
  <li><b>MAIÚSCULAS</b>: → <code>FOTO DE FÉRIAS</code></li>
  <li><b>Primeira letra maiúscula</b>: → <code>Foto de férias</code></li>
  <li><b>Cada Palavra Maiúscula</b>: → <code>Foto De Férias</code></li>
  <li><b>Cada Palavra, exceto de/da/e/of…</b>: → <code>Foto de Férias</code>. Palavras curtas de ligação (de, da, do, e, em, para, of, the, and, del, la, y…) ficam em minúsculas, a não ser no início do nome.</li>
</ul>
<p>Use "Aplicar em: Extensão" para mudar só a extensão.</p>`,
  },
  {
    id: "r-insert",
    title: "Inserir",
    sub: true,
    html: `
<p>Acrescenta algo ao nome.</p>
<h4>O quê</h4>
<ul>
  <li><b>Texto</b>: um texto fixo.</li>
  <li><b>Data atual</b>: a data de hoje.</li>
  <li><b>Data de modificação do arquivo</b>.</li>
  <li><b>Data em que a foto foi tirada</b>: lida dos dados EXIF da foto (JPEG, HEIC, PNG, WebP, TIFF e formatos RAW). Fotos sem essa data ficam como estão, a não ser que <b>"Sem essa data, usar a de modificação"</b> esteja marcado. Vídeos ainda não são lidos.</li>
  <li><b>Nome da pasta</b>: o nome da pasta onde o arquivo está.</li>
</ul>
${ex("IMG_1234.jpg", "2024-07-15 IMG_1234.jpg")}
<h4>Formato da data</h4>
<table>
  <tr><td><code>%Y</code> <code>%y</code></td><td>ano com 4 ou 2 dígitos (2024, 24)</td></tr>
  <tr><td><code>%m</code> <code>%d</code></td><td>mês e dia (07, 15)</td></tr>
  <tr><td><code>%H</code> <code>%M</code> <code>%S</code></td><td>hora, minuto, segundo</td></tr>
  <tr><td><code>%j</code></td><td>dia do ano (001–366)</td></tr>
</table>
<p>Exemplo: <code>%Y-%m-%d %H.%M.%S</code> dá <code>2024-07-15 14.32.10</code>. Uma <code>/</code> no formato vira <code>-</code>, porque não pode haver barra em nomes de arquivo.</p>
<h4>Onde</h4>
<ul>
  <li><b>No início</b> ou <b>No fim</b> do nome (antes da extensão).</li>
  <li><b>Na posição</b> N: antes do N-ésimo caractere (1 = no início).</li>
  <li><b>Na posição (contar do fim)</b> N: N caracteres antes do fim (0 = no fim, 1 = antes do último).</li>
</ul>`,
  },
  {
    id: "r-number",
    title: "Numerar",
    sub: true,
    html: `
<p>Numera os arquivos na ordem da lista.</p>
${ex("praia.jpg", "Viagem 01.jpg")}
<ul>
  <li><b>Modelo</b>: o novo nome, com variáveis:
    <ul>
      <li><code>{n}</code>: o número;</li>
      <li><code>{nome}</code>: o nome atual (sem extensão);</li>
      <li><code>{pasta}</code>: o nome da pasta onde o arquivo está.</li>
    </ul>
    Exemplos: <code>Viagem {n}</code>, <code>{nome} - {n}</code>, <code>{pasta} {n}</code>.</li>
  <li><b>Início</b> e <b>Passo</b>: 1 e 1 dão 1, 2, 3…; 10 e 10 dão 10, 20, 30…; passo negativo conta para trás.</li>
  <li><b>Dígitos</b>: completa com zeros à esquerda (2 dígitos: 01, 02… 10).</li>
  <li><b>Recomeçar a numeração em cada pasta</b>: cada pasta começa do início.</li>
</ul>
<p>A numeração segue a ordem da lista e conta só os arquivos visíveis no filtro. Na ordem por Novo nome, segue a ordem do nome atual. Para controlar a ordem, ordene por outra coluna ou use ⤒ ⤓.</p>`,
  },
  {
    id: "r-remove",
    title: "Remover caracteres",
    sub: true,
    html: `
<ul>
  <li><b>Espaços repetidos e nas pontas</b>: <code>  foto   de  férias </code> → <code>foto de férias</code></li>
  <li><b>Todos os espaços</b>: → <code>fotodeférias</code></li>
  <li><b>Acentos</b>: <code>Canção</code> → <code>Cancao</code></li>
  <li><b>Dígitos</b>: <code>foto123</code> → <code>foto</code></li>
  <li><b>Caracteres</b>: cada caractere digitado é removido onde aparecer. <code>_#</code> remove todos os <code>_</code> e <code>#</code>.</li>
</ul>`,
  },
  {
    id: "r-range",
    title: "Remover trecho",
    sub: true,
    html: `
<p>Apaga um pedaço do nome, definido pelo começo e pelo fim.</p>
<ul>
  <li><b>De</b>: o início do nome, uma posição, ou a primeira vez que um texto aparece.</li>
  <li><b>Até</b>: o fim do nome, uma posição, ou um texto (procurado depois do começo).</li>
  <li><b>Remover também os textos delimitadores</b>: se os próprios textos de início e fim também saem.</li>
</ul>
${ex("Filme [1080p] (2020).mkv", "Filme  (2020).mkv")}
<p>De texto <code>[</code> até texto <code>]</code>, removendo os delimitadores. Os textos diferenciam maiúsculas. Se um deles não aparece no nome, o nome fica como está.</p>`,
  },
  {
    id: "r-char",
    title: "Caractere em posição",
    sub: true,
    html: `
<p>Troca ou apaga o caractere numa posição.</p>
<ul>
  <li><b>Posição</b>: a partir de 1. Com <b>Contar do fim</b>, 1 é o último caractere.</li>
  <li><b>Só se for</b>: só muda se o caractere naquela posição for este (vazio = qualquer). Não diferencia maiúsculas, a não ser que <b>Diferenciar maiúsculas</b> esteja marcado.</li>
  <li><b>Trocar por</b>: o novo caractere. Vazio apaga.</li>
</ul>
${ex("_foto.jpg", "foto.jpg")}
<p>Posição 1, só se for <code>_</code>, trocar por vazio.</p>`,
  },
  {
    id: "r-ext",
    title: "Extensão",
    sub: true,
    html: `
<ul>
  <li><b>minúsculas</b> / <b>MAIÚSCULAS</b>: <code>foto.JPG</code> → <code>foto.jpg</code></li>
  <li><b>Definir</b>: troca a extensão por outra (<code>jpeg</code> → <code>jpg</code>). Não converte o arquivo, só muda o nome.</li>
  <li><b>Remover</b>: tira a extensão.</li>
</ul>
<p>Pastas não são afetadas por esta regra.</p>`,
  },
  {
    id: "preview",
    title: "Pré-visualização e conflitos",
    html: `
<p>A coluna <b>Novo nome</b> mostra o resultado das regras na hora. Nas linhas que mudam:</p>
<ul>
  <li>no <b>Nome atual</b>, o que vai sair aparece <span class="del">em vermelho, riscado</span>;</li>
  <li>no <b>Novo nome</b>, o que entra aparece <span class="ins">em azul</span>.</li>
</ul>
<p>Na primeira coluna, ● indica que o arquivo será renomeado, <span class="bad">!</span> vermelho indica um conflito e <span class="warn">!</span> amarelo indica um aviso. Passe o mouse para ver o motivo.</p>
<h4>Editar à mão</h4>
<p>Clique duas vezes no novo nome (ou selecione um arquivo e aperte <kbd>F2</kbd>) para escrever o nome que quiser. <kbd>Enter</kbd> confirma e <kbd>Esc</kbd> cancela. Um nome editado à mão ignora as regras; ↺ volta a usar as regras.</p>
<h4>Conflitos</h4>
<p>Arquivos com conflito não são renomeados. Os motivos são:</p>
<ul>
  <li>nome vazio, ou contendo <code>/</code>;</li>
  <li>nome com mais de 255 bytes;</li>
  <li>dois arquivos ficariam com o mesmo nome na mesma pasta;</li>
  <li>já existe um arquivo com esse nome fora da lista (nada é sobrescrito);</li>
  <li>no Windows: os caracteres <code>\\ : * ? " &lt; &gt; |</code>, ou ponto ou espaço no fim do nome.</li>
</ul>
<p>Em outros sistemas, esses caracteres do Windows só geram um aviso amarelo, desligável nas Opções. No Windows e no macOS, nomes que só diferem em maiúsculas contam como iguais.</p>`,
  },
  {
    id: "identical",
    title: "Cópias idênticas",
    html: `
<p>Depois que você adiciona arquivos, o programa procura, em segundo plano, arquivos da lista com <b>conteúdo idêntico</b>, mesmo que tenham nomes diferentes. Nada é apagado nem alterado: é só um aviso.</p>
<ul>
  <li>Arquivos com cópia ganham uma etiqueta com um <b>número de grupo</b> ao lado do nome: arquivos com o mesmo número são idênticos entre si, mesmo com nomes diferentes.</li>
  <li><b>Selecione um arquivo</b> e as cópias dele ficam destacadas na lista. Passar o mouse sobre a etiqueta mostra os nomes das outras cópias.</li>
  <li>O rodapé mostra quantos arquivos têm cópia. <b>Clique nele</b> para ver só esses arquivos, com as cópias lado a lado; clique de novo para ver todos. Daí você pode removê-los da lista ou renomeá-los à parte.</li>
  <li>Quando duas cópias iriam ficar com o mesmo nome, o conflito avisa que os arquivos são idênticos.</li>
</ul>
<p>A busca é feita em etapas para ser rápida: só arquivos do mesmo tamanho são comparados; depois, só o começo e o fim; o arquivo inteiro só é lido quando tudo isso coincide. Com arquivos grandes, o rodapé mostra o progresso. Arquivos vazios, pastas e atalhos (links simbólicos) ficam de fora.</p>`,
  },
  {
    id: "apply",
    title: "Renomear e desfazer",
    html: `
<p><b>Renomear</b> (<kbd>Ctrl</kbd>+<kbd>Enter</kbd>) aplica os novos nomes dos arquivos visíveis que mudam e não têm conflito. Antes, o programa pede confirmação (desligável nas Opções).</p>
<ul>
  <li>A renomeação é feita em duas etapas, passando por um nome temporário. Por isso dá para trocar nomes entre si (a ↔ b), e nenhum arquivo é sobrescrito.</li>
  <li>Pastas e arquivos de dentro delas podem ser renomeados juntos: primeiro o conteúdo, depois a pasta.</li>
  <li>Se um arquivo falhar (sem permissão, apagado por outro programa…), os outros seguem, e no fim aparece a lista do que falhou e por quê.</li>
  <li>Depois de renomear, as regras são desligadas (desligável nas Opções), para não serem aplicadas duas vezes.</li>
</ul>
<h4>Histórico</h4>
<p>Cada renomeação fica registrada no <b>Histórico</b>, mesmo depois de fechar o programa. Lá você pode:</p>
<ul>
  <li><b>Desfazer</b>: volta os nomes anteriores.</li>
  <li><b>Tentar de novo</b>: aparece quando parte do desfazer falhou (por exemplo, um arquivo foi movido depois). Devolva o arquivo ao lugar e tente de novo.</li>
  <li><b>✕</b>: exclui a entrada do histórico. Os arquivos não mudam, mas aquela renomeação não poderá mais ser desfeita.</li>
</ul>
<p class="tip"><kbd>Ctrl</kbd>+<kbd>Z</kbd> desfaz alterações na <i>lista</i> (adicionar, remover, mover, editar nome). Para desfazer uma renomeação já feita, use o Histórico.</p>`,
  },
  {
    id: "recipes",
    title: "Receitas",
    html: `
<h4>Fotos com a data em que foram tiradas</h4>
<p>Inserir → Data em que a foto foi tirada, formato <code>%Y-%m-%d %H.%M.%S </code>, no início.</p>
${ex("IMG_1234.jpg", "2024-07-15 14.32.10 IMG_1234.jpg")}
<h4>Numerar as fotos de várias pastas</h4>
<p>Adicione as pastas (com subpastas). Numerar → modelo <code>{pasta} {n}</code>, 3 dígitos, recomeçar em cada pasta.</p>
${ex("Praia/IMG_0001.jpg", "Praia/Praia 001.jpg")}
<h4>Limpar nomes de episódios</h4>
<p>Substituir texto <code>.</code> por espaço (Nome sem extensão), depois Remover trecho de texto <code> 720p</code> até o fim.</p>
${ex("Serie.S01E02.720p.WEB.mkv", "Serie S01E02.mkv")}
<h4>Padronizar extensões</h4>
<p>Extensão → minúsculas. Adicione Extensão → Definir <code>jpg</code> com o filtro <code>*.jpeg</code> para unificar.</p>`,
  },
  {
    id: "shortcuts",
    title: "Atalhos de teclado",
    html: `
<table>
  <tr><td><kbd>Ctrl</kbd>+<kbd>Enter</kbd></td><td>Renomear (funciona até digitando numa regra)</td></tr>
  <tr><td><kbd>Ctrl</kbd>+<kbd>Z</kbd></td><td>Desfazer a última alteração na lista</td></tr>
  <tr><td><kbd>Ctrl</kbd>+<kbd>O</kbd></td><td>Adicionar arquivos</td></tr>
  <tr><td><kbd>F2</kbd></td><td>Editar o novo nome do arquivo selecionado</td></tr>
  <tr><td><kbd>Delete</kbd></td><td>Remover os selecionados da lista</td></tr>
  <tr><td><kbd>Ctrl</kbd>+<kbd>A</kbd></td><td>Selecionar todos</td></tr>
  <tr><td><kbd>F1</kbd></td><td>Abrir esta ajuda</td></tr>
  <tr><td><kbd>Esc</kbd></td><td>Fechar janela ou menu</td></tr>
</table>
<p>No macOS, use <kbd>⌘</kbd> no lugar de <kbd>Ctrl</kbd>.</p>`,
  },
  {
    id: "options",
    title: "Opções",
    html: `
<p>A engrenagem abre as opções:</p>
<ul>
  <li><b>Idioma</b> e <b>Tema</b> (claro, escuro ou do sistema).</li>
  <li><b>Lembrar as regras ao fechar o programa</b>.</li>
  <li><b>Incluir arquivos ocultos ao adicionar pastas</b>.</li>
  <li><b>Pedir confirmação antes de renomear</b>.</li>
  <li><b>Desligar as regras depois de renomear</b>.</li>
  <li><b>Avisar sobre caracteres que o Windows não aceita</b>: útil para arquivos que vão para um pendrive ou para o Windows.</li>
</ul>
<p>As opções ficam em <code>~/.config/renome</code> (Linux), <code>%APPDATA%\\renome</code> (Windows) ou <code>~/Library/Application Support/renome</code> (macOS). O histórico fica em <code>~/.local/share/renome/history</code> no Linux e na mesma pasta das opções nos outros sistemas.</p>`,
  },
];
