//===============================================================================
//  #47 Edgar Stroulger
//===============================================================================

class  WCStroulger extends Bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardStroulgerTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Stroulgersmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardStroulgerBigTexture FILE=TEXTURES\menu\Folio\Cards\Stroulgerbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Edgar Stroulger";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=47
	bVendorsCanSell=true
	strVendorOwnedAfterGState="GSTATE110"
    skin(0)=Texture'HProps.skins.WizardCardStroulgerTex0'
	textureBig=Texture'WizCardStroulgerBigTexture'
	strDescriptionId="WizCard_0085"
}
