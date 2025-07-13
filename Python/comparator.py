import os
import hashlib
import multiprocessing
from collections import defaultdict

def hash_file(filepath, block_size=65536):
    """SHA256ハッシュを計算（chunked読み込み）"""
    sha = hashlib.sha256()
    try:
        with open(filepath, 'rb') as f:
            while chunk := f.read(block_size):
                sha.update(chunk)
        return sha.hexdigest()
    except Exception:
        return None

def _get_file_metadata(path):
    """ファイルのメタデータ（サイズと更新日時）を取得"""
    try:
        stat = os.stat(path)
        return (stat.st_size, stat.st_mtime)
    except FileNotFoundError:
        return None

def compare_all(folders, show_hidden=False):
    """
    複数フォルダ間での差分比較。
    まずファイルのメタデータ（サイズ、更新日時）で比較し、
    メタデータが異なる場合のみハッシュ計算を並列実行する。
    
    Args:
        folders: 比較対象のフォルダパスのリスト
        show_hidden: 隠しファイルを表示するかどうか
    """
    # 1. 全フォルダからファイルリストとメタデータを一度だけ取得
    all_files = defaultdict(list)
    for idx, folder_path in enumerate(folders):
        if not os.path.isdir(folder_path):
            continue
        for root, dirs, files in os.walk(folder_path):
            # 隠しフォルダのフィルタリング
            if not show_hidden:
                dirs[:] = [d for d in dirs if not d.startswith('.')]
            
            for name in files:
                # 隠しファイルのフィルタリング
                if not show_hidden and name.startswith('.'):
                    continue
                    
                full_path = os.path.join(root, name)
                rel_path = os.path.relpath(full_path, folder_path)
                metadata = _get_file_metadata(full_path)
                if metadata:
                    all_files[rel_path].append((idx, full_path, metadata))

    # 2. メタデータで一次比較し、ハッシュ計算が必要なファイルをリストアップ
    paths_to_hash = set()
    preliminary_results = {}
    sorted_rel_paths = sorted(all_files.keys())

    for rel_path in sorted_rel_paths:
        files_info = all_files[rel_path]
        
        # ファイルが1つしか存在しない場合は、ハッシュ不要
        if len(files_info) <= 1:
            continue

        # メタデータ（サイズ、更新日時）を比較
        first_metadata = files_info[0][2]
        if all(info[2] == first_metadata for info in files_info):
            preliminary_results[rel_path] = "一致 (メタデータ)"
        else:
            # メタデータが異なる場合はハッシュ計算リストに追加
            for _, full_path, _ in files_info:
                paths_to_hash.add(full_path)

    # 3. 必要なファイルハッシュだけを並列で計算
    hash_map = {}
    if paths_to_hash:
        paths_list = list(paths_to_hash)
        # プロセス数はCPUコア数か、ファイルが少ない場合はその数に合わせる
        num_processes = min(multiprocessing.cpu_count(), len(paths_list)) if paths_list else 1
        with multiprocessing.Pool(processes=num_processes) as pool:
            hashes = pool.map(hash_file, paths_list)
        hash_map = {path: h for path, h in zip(paths_list, hashes)}

    # 4. 最終的な結果を整理
    results = []
    for rel_path in sorted_rel_paths:
        files_in_folders = [None] * len(folders)
        hashes_in_folders = [None] * len(folders)
        
        # 各フォルダのファイル情報を整理
        for idx, full_path, _ in all_files[rel_path]:
            files_in_folders[idx] = full_path

        # ステータス判定
        if rel_path in preliminary_results:
            status = preliminary_results[rel_path]
        else:
            unique_hashes = set()
            for idx, path in enumerate(files_in_folders):
                if path:
                    h = hash_map.get(path)
                    hashes_in_folders[idx] = h
                    if h:
                        unique_hashes.add(h)
            
            if len(unique_hashes) <= 1:
                status = "一致"
            else:
                status = "差異"

        # ファイル情報を収集（ソート用）
        file_info = []
        for i in range(len(folders)):
            path = files_in_folders[i]
            hash_val = hashes_in_folders[i]
            if path and os.path.exists(path):
                stat = os.stat(path)
                size = stat.st_size
                mtime = stat.st_mtime
            else:
                size = 0
                mtime = 0
            
            file_info.append({
                'folder': folders[i],
                'path': path,
                'hash': hash_val,
                'size': size,
                'mtime': mtime
            })
        
        entry = {
            'rel_path': rel_path,
            'status': status,
            'files': file_info
        }
        results.append(entry)

    return results
