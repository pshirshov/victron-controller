// @ts-nocheck
import {BaboonGeneratedLatest, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'

export class VersionInfo implements BaboonGeneratedLatest {
    private readonly _current_version: string;
    private readonly _min_supported_version: string;
    private readonly _git_sha: string | undefined;

    constructor(current_version: string, min_supported_version: string, git_sha: string | undefined) {
        this._current_version = current_version
        this._min_supported_version = min_supported_version
        this._git_sha = git_sha
    }

    public get current_version(): string {
        return this._current_version;
    }
    public get min_supported_version(): string {
        return this._min_supported_version;
    }
    public get git_sha(): string | undefined {
        return this._git_sha;
    }

    public toJSON(): Record<string, unknown> {
        return {
            current_version: this._current_version,
            min_supported_version: this._min_supported_version,
            git_sha: this._git_sha !== undefined ? this._git_sha : undefined
        };
    }

    public with(overrides: {current_version?: string; min_supported_version?: string; git_sha?: string | undefined}): VersionInfo {
        return new VersionInfo(
            'current_version' in overrides ? overrides.current_version! : this._current_version,
            'min_supported_version' in overrides ? overrides.min_supported_version! : this._min_supported_version,
            'git_sha' in overrides ? overrides.git_sha! : this._git_sha
        );
    }

    public static fromPlain(obj: {current_version: string; min_supported_version: string; git_sha: string | undefined}): VersionInfo {
        return new VersionInfo(
            obj.current_version,
            obj.min_supported_version,
            obj.git_sha
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return VersionInfo.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return VersionInfo.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#VersionInfo'
    public baboonTypeIdentifier() {
        return VersionInfo.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.1.0", "0.2.0", "0.3.0"]
    public baboonSameInVersions() {
        return VersionInfo.BaboonSameInVersions
    }
    public static binCodec(): VersionInfo_UEBACodec {
        return VersionInfo_UEBACodec.instance
    }
}

export class VersionInfo_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: VersionInfo, writer: BaboonBinWriter): unknown {
        if (this !== VersionInfo_UEBACodec.lazyInstance.value) {
          return VersionInfo_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeString(buffer, value.current_version);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeString(buffer, value.min_supported_version);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.git_sha === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                BinTools.writeString(buffer, value.git_sha);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            BinTools.writeString(writer, value.current_version);
            BinTools.writeString(writer, value.min_supported_version);
            if (value.git_sha === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                BinTools.writeString(writer, value.git_sha);
            }
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): VersionInfo {
        if (this !== VersionInfo_UEBACodec .lazyInstance.value) {
            return VersionInfo_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 3; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const current_version = BinTools.readString(reader);
        const min_supported_version = BinTools.readString(reader);
        const git_sha = (BinTools.readByte(reader) === 0 ? undefined : BinTools.readString(reader));
        return new VersionInfo(
            current_version,
            min_supported_version,
            git_sha,
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return VersionInfo_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return VersionInfo_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#VersionInfo'
    public baboonTypeIdentifier() {
        return VersionInfo_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new VersionInfo_UEBACodec())
    public static get instance(): VersionInfo_UEBACodec {
        return VersionInfo_UEBACodec.lazyInstance.value
    }
}