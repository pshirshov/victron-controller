// @ts-nocheck
import {BaboonGeneratedLatest, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'

export class CoreState implements BaboonGeneratedLatest {
    private readonly _id: string;
    private readonly _depends_on: Array<string>;
    private readonly _last_run_outcome: string;
    private readonly _last_payload: string | undefined;

    constructor(id: string, depends_on: Array<string>, last_run_outcome: string, last_payload: string | undefined) {
        this._id = id
        this._depends_on = depends_on
        this._last_run_outcome = last_run_outcome
        this._last_payload = last_payload
    }

    public get id(): string {
        return this._id;
    }
    public get depends_on(): Array<string> {
        return this._depends_on;
    }
    public get last_run_outcome(): string {
        return this._last_run_outcome;
    }
    public get last_payload(): string | undefined {
        return this._last_payload;
    }

    public toJSON(): Record<string, unknown> {
        return {
            id: this._id,
            depends_on: this._depends_on,
            last_run_outcome: this._last_run_outcome,
            last_payload: this._last_payload !== undefined ? this._last_payload : undefined
        };
    }

    public with(overrides: {id?: string; depends_on?: Array<string>; last_run_outcome?: string; last_payload?: string | undefined}): CoreState {
        return new CoreState(
            'id' in overrides ? overrides.id! : this._id,
            'depends_on' in overrides ? overrides.depends_on! : this._depends_on,
            'last_run_outcome' in overrides ? overrides.last_run_outcome! : this._last_run_outcome,
            'last_payload' in overrides ? overrides.last_payload! : this._last_payload
        );
    }

    public static fromPlain(obj: {id: string; depends_on: Array<string>; last_run_outcome: string; last_payload: string | undefined}): CoreState {
        return new CoreState(
            obj.id,
            obj.depends_on,
            obj.last_run_outcome,
            obj.last_payload
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return CoreState.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return CoreState.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#CoreState'
    public baboonTypeIdentifier() {
        return CoreState.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.2.0"]
    public baboonSameInVersions() {
        return CoreState.BaboonSameInVersions
    }
    public static binCodec(): CoreState_UEBACodec {
        return CoreState_UEBACodec.instance
    }
}

export class CoreState_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: CoreState, writer: BaboonBinWriter): unknown {
        if (this !== CoreState_UEBACodec.lazyInstance.value) {
          return CoreState_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeString(buffer, value.id);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeI32(buffer, Array.from(value.depends_on).length);
            for (const item of value.depends_on) {
                BinTools.writeString(buffer, item);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeString(buffer, value.last_run_outcome);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.last_payload === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                BinTools.writeString(buffer, value.last_payload);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            BinTools.writeString(writer, value.id);
            BinTools.writeI32(writer, Array.from(value.depends_on).length);
            for (const item of value.depends_on) {
                BinTools.writeString(writer, item);
            }
            BinTools.writeString(writer, value.last_run_outcome);
            if (value.last_payload === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                BinTools.writeString(writer, value.last_payload);
            }
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): CoreState {
        if (this !== CoreState_UEBACodec .lazyInstance.value) {
            return CoreState_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 4; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const id = BinTools.readString(reader);
        const depends_on = Array.from({ length: BinTools.readI32(reader) }, () => BinTools.readString(reader));
        const last_run_outcome = BinTools.readString(reader);
        const last_payload = (BinTools.readByte(reader) === 0 ? undefined : BinTools.readString(reader));
        return new CoreState(
            id,
            depends_on,
            last_run_outcome,
            last_payload,
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return CoreState_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return CoreState_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#CoreState'
    public baboonTypeIdentifier() {
        return CoreState_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new CoreState_UEBACodec())
    public static get instance(): CoreState_UEBACodec {
        return CoreState_UEBACodec.lazyInstance.value
    }
}